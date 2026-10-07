# String Collections v1

Audience: language users, agent authors, and compiler contributors.

Status: Partial — implemented for the reference interpreter and generated C11
(`run`, `run --native`, `build --target native`). Every Core Wasm lane refuses
the family with one stable diagnostic (`SPX-W116`). It builds on
[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md) and
[Text Toolkit v1](TEXT-TOOLKIT-V1.md).

## Objective

Counting by key, distinct values, and ranking were out of reach: a program
could cut text into fields, but it had nowhere to keep a value per field and
no ordering of strings. v1 adds one deterministic string-keyed map,
`Map<string, i64>`, and bytewise string ordering. That covers distinct
counts, per-key counts and sums, and top-N rankings without a general
collection or sort.

## The type

`Map<string, i64>` is the only admitted instantiation; any other spelling,
including a bare `Map`, is `SPX-T274`, and declaring a type named `Map` is
`SPX-S113`. A map is uniquely owned and never `Copy`. Its entries are kept in
ascending bytewise key order (the order of `string_compare`), so iteration and
every observable result are deterministic and identical on every backend.

A map is a **local binding**. It is created by `map_new` as a `let`
initializer, changed only by the same-owner updates below, and passed only as
the first argument of a map operation. Everything else is `SPX-T275`: a
function parameter or result, a record, class, or variant field, a second
binding (`let other = counts;`), a branch, match, or block result, a closure
capture, a temporary map (`map_len(map_new(4usize))`), and a `map_add` that is
not a same-owner update.

```text
let mut counts = map_new(256usize);
counts = map_add(counts, key, 1);
let n = map_get_or(counts, key, 0);
```

## Operations

All are compiler-owned and reserved (declaring one is `SPX-S113`). They
resolve to ordinary monomorphic calls with the stable identities below and
take no type arguments (`SPX-T225`); no prelude bytes, graph schema version,
or earlier program's projection changes.

| Function | Stable identity | Signature |
| --- | --- | --- |
| `string_compare` | `core.string.compare` | `(a: string, b: string) -> i64` |
| `map_new` | `core.map.new` | `(capacity: usize) -> Map<string, i64>` |
| `map_add` | `core.map.add` | `(map: own Map<string, i64>, key: string, delta: i64) -> Map<string, i64>` |
| `map_set` | `core.map.set` | `(map: own Map<string, i64>, key: string, value: i64) -> Map<string, i64>` |
| `map_get_or` | `core.map.get_or` | `(map: Map<string, i64>, key: string, default: i64) -> i64` |
| `map_has` | `core.map.has` | `(map: Map<string, i64>, key: string) -> bool` |
| `map_len` | `core.map.len` | `(map: Map<string, i64>) -> usize` |
| `map_key_at` | `core.map.key_at` | `(map: Map<string, i64>, index: usize) -> string` |
| `map_value_at` | `core.map.value_at` | `(map: Map<string, i64>, index: usize) -> i64` |

- `string_compare` is `-1`, `0`, or `1` by unsigned byte order, a proper
  prefix ordering first. Equality agrees with `==` on strings.
- `map_new` creates an empty map that holds at most `capacity` entries;
  `capacity` may be at most 65,536. Storage grows with the entries, so a large
  capacity costs nothing until it is used.
- `map_add` inserts `key` with `delta`, or adds `delta` to its value with
  checked `i64` arithmetic. `map_set` inserts or replaces.
- `map_get_or` returns the key's value or `default`; `map_has` tests
  membership; `map_len` counts entries.
- `map_key_at` and `map_value_at` read the entry at `index` in ascending key
  order, for `index < map_len(map)`. `map_key_at` returns a new string.

### Status domain

`semaprax.map.v1`, adapter class, never retryable:

| Code | Meaning |
| ---: | --- |
| 1 | a new key does not fit: the map already holds `capacity` entries |
| 2 | `map_key_at` / `map_value_at` index is not below `map_len` |
| 3 | `map_new` capacity is above 65,536 |
| 4 | `map_add` would overflow the entry's `i64` value |

A failure is the ordinary checked language status: it is sticky, cleanup
releases every live value exactly once, and no partial result is published.
`map_add` and `map_set` check before they take the map, so a failed update
leaves the map in its call-argument slot for ordinary region cleanup.

## Ownership

`map_add` and `map_set` are the fourth same-owner reopen, after
`values = vec_push<T>(values, v)`, `buffer = bytes_set(buffer, i, v)`, and the
String append. They share the append protocol of
[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md): the first operand moves the
binding's current generation into the call's argument epoch, the assignment
publishes the returned map into the then-dead slot, and the binding keeps its
cleanup position, so loop iterations and branch joins see one stable
initialization history. The CleanupPlan carries ordinary `transfer`,
`call_commit`, and `initialize` facts; the leaf lifecycle is `core.map.drop`.
A later operand that reads the map (`m = map_add(m, k, map_len(m))`) is not
the reopen and reports the ordinary moved-value diagnostic (`SPX-O101`).

Every other operand is borrowed: a map operand aliases its binding, and a key
is read without being consumed — the map copies a key only when it inserts it.
Like the other `string_*` operations, a borrowed `string` binding operand may
be read through a clone that the call's cleanup region releases.

### Loops

The operations are admitted in `while` and `for` bodies, including inside
`if` branches, exactly like the String append: an outer `let mut` map is
updated in the body with `counts = map_add(counts, key, 1);`, and a map
created in the body is released at the end of its iteration. A `while`
condition may call `map_len` and `map_value_at`, which allocate nothing; any
call that touches a string there (including `map_has` and `map_get_or` with a
key) is `SPX-T252`. Iterate a map by index:

```text
let mut index = 0usize;
while index < map_len(counts) {
    total = total + map_value_at(counts, index);
    index = index + 1usize;
    0
}
```

## Ranking without sorting

Entries are in key order, so "ties by key ascending" is "ties by index
ascending". The top `n` entries by value descending are `n` scans that each
pick the best entry strictly after the previous pick in that order:

```text
let after = shown == 0 || c < previous_count || c == previous_count && i > previous_index;
```

## Backends

| Backend | Behavior |
| --- | --- |
| Reference interpreter | Executes all nine; a map is a sorted entry vector. |
| Native C11 | Executes all nine in length-delimited String profiles; a map is one `spx_map_v1 *` carrier with sorted owned keys and binary search. |
| Core Wasm (every lane) | `SPX-W116` before any operand is emitted, naming the first collection operation. |

## Evidence

`tests/language/string_collections_v1.rs` round-trips the corpus through the
canonical formatter and the graph (asserting each `callee` identity, the
`string_map` type, and the `core.map.drop` lifecycle), runs it on the
interpreter and on C11 at `-O0` and `-O2` under an allocation-counting
allocator that requires zero live allocations after every case — including
full maps, out-of-range indexes, and overflow inside loops — pins the Wasm
refusal and the placement diagnostics, and drives a command-line word count
through `semaprax run` and `semaprax run --native`.

## Not in v1

Other key and value types, sets (use a map and `map_len`), removal, sorting of
vectors, maps as parameters, results, or fields, `<` on strings, and the Core
Wasm lowering.
