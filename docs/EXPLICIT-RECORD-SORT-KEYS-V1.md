# Explicit Record Sort Keys v1

Status: locally qualified for the bounded v1 profile.
Audience: coding agents, compiler contributors and reviewers.

`vec_sort_by<Row>(rows, "priority:desc,arrival:asc")` is a consuming operation
on an already admitted flat Copy-record or directly owned String/Bytes-record
Vec. It consumes one owner and returns that owner at exactly its next generation.
`vec_sort` and `vec_sort_owned` retain their frozen declaration-order comparison.
The policy never changes field declaration order, JSON wire order or row layout.

## Closed source and authority

The type is an explicit, monomorphic record with one to eight explicit fields,
selected by the existing Copy-record or owned-leaf classifier and its unchanged
capacity. Generic, nested, recursive, implicit-ID records and primitive Vecs are
refused. The sole policy argument is a literal of 1..8 distinct `name:asc` or
`name:desc` keys separated by commas, without whitespace, at most 1024 UTF-8
bytes. A field name uses ordinary ASCII identifier spelling. Empty, duplicate,
foreign, computed or malformed policies are `SPX-T311` source refusals.

The literal is compiler metadata, not an evaluated or allocated String. The
runtime Call has exactly one owning Vec operand and one type argument, no
instance. Its callee begins `core.vec.sort-by.v1:` and carries a canonical,
length-delimited descriptor bounded to 16384 bytes: element type identity,
complete layout identity, decimal key count, and ordered persistent field IDs
with `a`/`d` directions. Each string is decimal byte length, colon, exact bytes;
counts have no leading zeros. Layout includes family, nominal identity, exact
capacity, and every field ID/type in declaration order. Each backend and HIR
validation replays this complete descriptor from declarations. Workspace source
reconstruction independently derives the same callee at each source identity.
Reserved prefixes alone authorize nothing; authored declarations/imports and
forged, stale, duplicate, truncated or trailing descriptor data fail closed.
Generated identities and descriptor buffers retain ordinary bounded accounting.

New programs select prelude v15, binding this contract. Programs without explicit
keys retain their prior prelude selection and bytes. Older source/graph profile
labels cannot authorize a new carrier or erase the new prelude contract. No
new capability, public record ABI or effectful comparator callback is granted.

## Comparison, transfer and storage

Keys compare lexicographically in selected order. Scalar keys use the existing
Vec total order: numeric signed/unsigned, bool/char, floating IEEE bit total order
including signed zeros and NaN payloads. String and Bytes compare unsigned exact
bytes then length, including NUL, UTF-8 prefixes and non-ASCII bytes. Descending
reverses comparator operands, never negates signed integers or subtraction.
Rows with equal selected keys retain their input order, irrespective of
unselected fields. Empty, singleton and full-capacity sorts have the same rule.

Sort authenticates the same row storage and authority entry before mutation. It
allocates no replacement row array, clones no owned payload and drops no row.
A bounded stack carrier temporarily holds one row during stable native insertion;
interpreter and private Core-Wasm perform adjacent in-place row swaps. Each
sort advances generation once at the ordinary infallible Call commit, preserving
left-to-right argument staging, canonical cleanup and sticky selected failures.
Views/loans of this Vec prevent sort until their last use, including aliases.

The private Core-Wasm `env.spx_vec_sort_by_v1` host call accepts owner, authenticated
layout identity/shape, and <=8 packed field/direction bytes. Each byte encodes
`2*(declaration_position+1)+descending`; zero terminates the shorter list.
A zero policy is only a Copy constructor layout bind and does not renew the
owner. A nonzero policy authenticates layout, distinct fields, scalar/owned
family, storage and epoch; it sorts existing storage and renews exactly once.
An invalid result or exhausted epoch is an invariant trap, not a Vec/3 result.
This private host fixture is an owning executable gate, not public host support.

[Borrowed Record-Field Uniqueness v1](BORROWED-RECORD-FIELD-UNIQUENESS-V1.md)
reuses only this layout/field derivation for its distinct borrowed
`core.vec.unique-by.v1` descriptor; it never selects this consuming opcode or
renews an owner. When that operation is present, Copy constructors also bind
their layout through the zero policy above.

## Owning regressions and qualification

Owning checks cover independent multiple-ranking policies over
the same wire layout; stable ties; exact scalar bounds and floating bits;
unsigned String/Bytes prefix/NUL order; empty/singleton/full vectors; source,
canonical, HIR, graph and cache replay; forged identity/layout/type/direction;
and live-loan/profile refusal. Generated C runs at O0 and O2 with settlement and
sort allocation observers. The interpreter and private Core-Wasm execute
the same source. All 59 focused source, HIR, graph, cache, help and backend
checks passed. The native probes verified no replacement allocation or
payload clone during the sort. Agent token, turn and cost effects remain
unmeasured.

## Catalog adoption

The scoped Catalog successor sorts its existing response `Item` rows with
`department:asc,priority:asc,id:asc,stock:asc,mark:asc`. The response declaration
retains `id,department,priority,stock,mark` wire order. Selection constructs the
final row once and accumulates bounded metrics, after validating all request
rows including excluded rows. The former `SortItem` carrier and second Vec
rebuild are retained verbatim only as an independent owning test reference.
The returned output Vec retains the input-count selection capacity; the old
second rebuild used selected-count capacity. The current and archived
pipelines both passed the frozen 23-case raw-wire oracle at O0 and O2.
The archived pipeline uses the existing factored comparison for its large
ordinary cleanup replay. That comparison independently verifies the exact
HIR and plan observation languages under the existing limits. The complete
ShiftSim15 corpus also passed at its ordinary limits.
