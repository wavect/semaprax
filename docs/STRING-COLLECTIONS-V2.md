# String Collections v2

Audience: language users and compiler contributors.

Status: authored; focused execution checks are deferred until the complete OPT
implementation batch is integrated. This is an additive compiler collection
profile, not a public generic host ABI.

## Types and ownership

`Map<K,V>` accepts keys `string`, `i64`, or `bool`. Values may be `string`
or any Copy scalar: `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, `bool`.
`Set<K>` accepts the same three key types. Both are uniquely owned, never Copy.
Bare types, unsupported arguments and authored declarations shadowing these
compiler types remain errors.

Collections can move through explicit `own` parameters and results. Explicit
`borrow` parameters permit reads without consuming the collection. Bounded
monomorphic record fields may hold collections alongside String, Bytes and Copy
scalars, including nested admitted records. Record identities and field identities
remain authenticated. Generic, resource, view and invariant-bearing records are
outside this executable record profile; unchecked invariants are never admitted.

```text
@id("counts.add")
fn add(counts:own Map<string,i64>, key:string)->Map<string,i64> {
  map_add(counts,key,1)
}
@id("counts.read")
fn count(counts:borrow Map<string,i64>, key:string)->i64 {
  map_get_or(counts,key,0)
}
```

The older unparameterized operations still select `Map<string,i64>` and retain
their `core.map.*` identities and `semaprax.map.v1` status domain. V2 lifts their
local-only transport restriction and adds `map_remove(map,key)`. Explicit generic
operations use separate `core.collection.*.v2` identities. Generic
`Map<string,i64>` retains its legacy carrier and status domain.

## Operations

All generic calls require explicit type arguments. The first argument of a
mutation is consumed; its result owns the successor. Read operations borrow
their collection. String keys and values are borrowed on input and copied into
entries. String read results are fresh owners, so they survive removal or
destruction of the collection.

| Operation | Result and behavior |
| --- | --- |
| `map_new<K,V>(capacity)` | Empty `Map<K,V>` with a `usize` capacity limit |
| `map_set<K,V>(map,key,value)` | Insert or replace, returning the successor map |
| `map_add<K,i64>(map,key,delta)` | Insert or checked addition; only `i64` values |
| `map_remove<K,V>(map,key)` | Remove if present, returning the successor map |
| `map_get_or<K,V>(map,key,fallback)` | Stored value or fallback |
| `map_has<K,V>(map,key)` | Boolean membership |
| `map_len<K,V>(map)` | Entry count as `usize` |
| `map_key_at<K,V>(map,index)` | Key at a checked `usize` index |
| `map_value_at<K,V>(map,index)` | Value at that same ordered index |
| `set_new<K>(capacity)` | Empty `Set<K>` |
| `set_insert<K>(set,key)` | Insert if absent, returning the successor set |
| `set_remove<K>(set,key)` | Remove if present, returning the successor set |
| `set_has<K>(set,key)` | Boolean membership |
| `set_len<K>(set)` | Unique key count as `usize` |
| `set_key_at<K>(set,index)` | Key at a checked ordered index |

```text
let mut labels=map_new<i64,string>(8usize);
labels=map_set<i64,string>(labels,2,"second");
labels=map_set<i64,string>(labels,-1,"first");
let saved=map_get_or<i64,string>(labels,2,"missing");
labels=map_remove<i64,string>(labels,2);
let mut seen=set_new<string>(8usize);
seen=set_insert<string>(seen,saved);
seen=set_insert<string>(seen,saved);
let unique=set_len<string>(seen);
```

Indices enumerate ascending unsigned UTF-8 bytes for String keys, signed
numeric order for `i64`, and `false` before `true` for Boolean keys. Replacing
a value leaves its key position unchanged. Removing a missing key and inserting
an existing set key are successful no-ops. Capacity counts live distinct keys;
removal makes that space available again. Capacity is bounded at 65,536.

## Failures and settlement

Typed collections use `semaprax.map.v2`; the legacy String/i64 map uses
`semaprax.map.v1`. Both have the same codes:

| Code | Meaning |
| ---: | --- |
| 1 | A new distinct key exceeds capacity |
| 2 | An indexed read is outside the live entry count |
| 3 | Requested capacity exceeds 65,536 |
| 4 | `map_add` would overflow `i64` |

Failure is sticky. Arguments evaluate left to right, owned calls commit at
their authenticated boundary, and canonical cleanup releases each live owner
exactly once. String allocation, copying and collection destruction use the
existing checked lifecycle. A host adapter cannot replace the selected status
or publish a partial result.

## Projection and verification

The additive Prelude v13 builds on sorting's v12 and published record-iterator
v11; earlier contract bytes stay frozen. Source and independent HIR admission
authenticate the exact closed collection arguments. Compiler operation types,
record layouts, cleanup/replay and backend carriers agree on ownership.

The pending focused corpus lives in `tests/language/map_collections_v2.rs`;
Wasm provider and hostile-HIR cases are owned by the corresponding collection
modules. Project v25 Stream Text helper transport must use the same exact
collection and record admission. Completion requires those selected executable
checks, including deterministic order, copies surviving removal, failures,
settlement, function transport and record transport. It does not imply a public
generic ABI, arbitrary collection nesting or hosted validation.
