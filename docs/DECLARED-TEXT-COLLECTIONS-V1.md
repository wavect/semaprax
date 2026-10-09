# Declared Text Collections v1

Status: declaration checking and hostile gates authored; execution/profile and
application qualification pending. This is a logical type declaration rule,
not runtime support for `Vec<string>` or a String-bearing record vector.

Ordinary record fields can name `Vec<string>` and `Vec<R>` where `R` has an
explicit persistent identity, no type parameters, 1–8 direct fields, exactly
one `string` field and only the existing Vec scalar types in its other fields.
For example, the faithful scheduling schema can declare:

```text
Patient { id: string, arrival: i64, service: i64, priority: i64, deadline: i64 }
Request { servers: Vec<string>, patients: Vec<Patient> }
```

Actual source must declare all persistent identities in the ordinary way.
Names, schema derivation and generated-source provenance grant no exemption.
Unknown, nested, generic, multi-String and resource element shapes retain their
existing declaration refusals.

The source classifier in `source_verify/declared_type/declared_collection.rs`
and independent HIR classifier in `hir/declared_collection.rs` agree on that
shape. Type facts establish non-Copy, sized, resource-free logical ownership
requiring settlement, with a `declared-vector-only:` identity key. This key
has no byte size, backend layout or cleanup/runtime carrier authority. It is
deliberately distinct from executable `vec:` type facts. Ordinary records that
refer to these fields transitively inherit their non-Copy ownership facts.
Canonical formatting, semantic graph and retained source facts carry the real
declared element types unchanged.

Every executable occurrence is refused: direct or transitive function
parameters/results, yielded requests/responses, local type annotations,
constructors and closures. All function bodies are checked, including uncalled
functions and unreachable branches. Source reports `SPX-T281`; independent HIR
replay reports `SPX-H006` before backend emission. The existing unsupported Vec
operation gates remain intact. Forged cached Copy facts, changed declaration
origins and source/index disagreement cannot supply runtime authority.

A checked derivation may read this logical schema and emit explicitly different
executable view types. For example, an identifier span stores byte offsets into
caller-owned normalized JSON, while the patient view carries those offsets and
the scalar values. Those view records and their bounded collections must pass
ordinary source/HIR/profile verification. They never become a decoded owned
`Request`; the backing bytes must outlive every view use. Derivation must retain
the exact authored schema and preserve the original application requirements.

Project v29 retains only its independently authenticated runtime nominal
closure. Logical declarations remain ordinary checked source; if referenced by
a function, they are refused before that reachability selection can hide an
invalid use. Old profile names and public ABIs remain closed.

`hir::declared_collection::tests` owns declaration/canonical/graph replay,
stale source rejection, exact affine facts, unused-runtime-use refusal, forged
declaration origin/fields/cache facts and backend refusal. The application JSON
derivation corpus must additionally qualify a real Project containing this
schema and its generated executable views. No benchmark or runtime collection
support follows from declaration admission alone.
