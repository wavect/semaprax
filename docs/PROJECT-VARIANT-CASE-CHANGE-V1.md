# Project Variant Case Change v1

Status: **Partial; library implementation and regression sources authored and
unrun.**

Audience: compiler contributors and agents editing immutable Project candidates.

`add_variant_case` appends one explicitly identified case containing exactly
one owned `Bytes` field to an existing variant. Canonical `.spx` remains the
authority. The operation does not construct the new case, add or rewrite a
match arm, edit an existing constructor, or grant source/publication authority.

## Closed request

```json
{
  "kind": "add_variant_case",
  "target": "message.kind",
  "case": {
    "id": "message.kind.data",
    "name": "Data",
    "field": {
      "id": "message.kind.data.bytes",
      "name": "payload",
      "type": "Bytes"
    }
  }
}
```

The intention, case, and field objects are closed. Case and field identities
use the existing bounded explicit-ID grammar. Their display names use the
bounded source identifier grammar. Both identities and both display names must
be fresh in the authenticated authored type-member inventory.

The structural constructor schema also names `string` so the compiler can
return the stable semantic refusal `SPX-G516`. It is not an admitted v1 field
type. String-bearing variants remain outside the existing interpreter, native,
and Core Wasm flat owned-variant profile; this operation does not widen that
runtime boundary.

## Eligibility and source transformation

The target must be an explicit monomorphic source variant whose retained
compiler TypeFacts prove it is Copy, drop-free, sized, and resource-free. It
must contain fewer than 64 cases. At least one exact existing constructor must
occur in authenticated Project source, and no exact variant pattern selecting
the target may occur in any function or class-method body, guard, contract,
loop, or nested expression. Imported type aliases are resolved through their
persistent nominal identity rather than their display spelling.

Every source module is authenticated against its retained path, module,
source revision, and source digest. The target declaration and complete old
case/member prefix must agree with retained HIR identities, names, positions,
and ownership. Generic variants, already-owning variants, records, classes,
resources, absent constructors, target patterns, empty or multiple fields,
and scalar, nominal, borrowed, String, or resource payloads fail closed.

The compiler appends the requested case after every existing declaration case.
It does not touch any expression. Thus all existing constructors, their field
evaluation, failure order, and surrounding lazy/control position remain exact.
No default value, constructor call, match arm, wildcard handler, or other
source is invented.

## Independent admission and replay

The complete candidate is canonically formatted, reparsed, rebuilt, and
independently replayed through ordinary Project validation. Existing
interpreter, native C, and Core Wasm admission remains mandatory; the operation
adds no backend exception. After rebuild, an independent reconstruction must
byte-equal every candidate source.

The checked target must preserve the exact ordered old case prefix, including
each case and field identity, name, index, and resolved type. It must append
exactly one case and one `Bytes` field with the requested identities and names.
Retained TypeFacts must change exactly from Copy/drop-free/sized/resource-free
to non-Copy/needs-drop/sized/resource-free. Cleanup and layouts are rebuilt by
their ordinary compiler owners; this operation never copies, sorts, or repairs
their vectors.

Candidate replay and recovery bind the full intention bytes. Rebase and merge
fingerprint the exact source variant and retained HIR case/field inventory,
including resolved nominal identity keys for old payload types. A concurrent
shape, owner, binding, identity, or resolved-type change conflicts. Unrelated
function changes may replay, while two additions to the same variant conflict
conservatively even if their requested identities differ.

## Diagnostics, bounds, and evidence

- `SPX-G516` owns invalid or unsupported request/eligibility shapes.
- `SPX-G517` owns retained source/HIR authentication and reconstruction failures.
- `SPX-G518` owns case, traversal, TypeFacts, and source-output bounds.

The variant has at most 64 final cases. IDs and names are at most 128 bytes;
pattern inspection has depth 256; complete candidate JSON/source and compiler
TypeFacts retain their existing tighter shared limits.

Authored regressions in `tests/project_candidate/variant_case.rs` cover Bytes
success, exact source/graph/replay, interpreter/native/Wasm admission, String
and pattern refusal, atomic failure, unrelated merge replay, and competing-case
conflict. Constructor and v5 response schemas are covered in their existing
schema-test modules. These sources were not executed for this change.

This v1 does not claim String-variant support, construction of the new case,
exhaustive-handler migration, ABI compatibility, runtime equivalence,
performance equivalence, execution evidence, source writes, or publication.
