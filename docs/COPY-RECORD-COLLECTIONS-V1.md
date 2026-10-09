# Copy Record Collections v1

Status: implementation and executable regressions authored; focused execution,
hosted qualification and matched agent campaigns remain pending. This is the
ordinary pure-core tranche of OPT-723, not closure of that issue or proof of
agent token/cost savings.

## Element and operation contract

`R` is an explicitly identified, monomorphic record with one through eight
direct fields. Each field is exactly `i64`, `i32`, `u8`, `usize`, `char`, `f32`,
`f64`, or `bool`. Declaration and field identities remain nominal. Empty,
nested, generic and owned records are outside this element profile. Existing
record invariants execute at ordinary checked construction; copying and
reordering an already checked record preserves its field values.

`Vec<R>` uses the existing explicitly typed `vec_with_capacity`, `vec_push`,
`vec_len`, `vec_capacity`, `vec_get`, `vec_reserve_exact`, `vec_set`, `vec_clear`
and `vec_sort` spellings and stable operation identities. The carrier remains
one affine owner. Push and set copy their `R` argument, and get returns a Copy
`R`; reusing the original record is valid. Owner arguments stage left to right
and settle through the ordinary grouped call commit and canonical cleanup plan.
Private pure helpers may borrow the vector and accept or return `R` by value.
Indexed while loops admit checked record construction, field projection, get,
set, push and sorting. `for`/`for own` and generic collection wrappers retain
their previous element restrictions.

Sorting is stable lexicographic order in declaration-field order. Integer and
character fields use their scalar order, false precedes true, and floating
fields use the exact scalar `total_cmp` bit order, including signed zero and
NaN sign/payload ordering. No comparison callback or user effect executes.

Each record capacity slot charges eight bytes per field. The existing 65536-byte
scalar storage envelope therefore allows `floor(8192 / field_count)` records;
no storage cap is raised. Length and capacity count records, not physical words.
Reserve's argument is additional capacity: `max(old_capacity, len + additional)`.
Overflow or a request exceeding the record limit selects `semaprax.vec.v1` code
3. Full push selects code 1; get/set outside initialized length select code 2.
Get/set check the logical index before multiplying by the field count. Failure
is sticky and an unsuccessful invocation does not publish its result.

## Independent checks and representations

`source_verify/declared_type/copy_record_collection.rs` owns the source shape;
`hir/copy_record_collection.rs` independently reconstructs it from authenticated
DeclarationIndex facts. Neither broadens the scalar-only element predicate.
HIR validation, ownership/cleanup replay, native and Wasm carrier admission use
this independently authenticated union. No new parser syntax, HIR expression,
graph node or cache field is introduced. Canonical source and graph retain the
ordinary nominal types and typed calls; a cached plan is replayed against those
same declaration facts. Prelude operation/version selection remains unchanged.

The interpreter stores checked records. Native code and Core Wasm use a bounded
array of scalar words in one existing authenticated tag-1 Vec allocation.
Compiled typed loads/stores pack and unpack declaration fields; this physical
representation is private, and does not grant a record/public host ABI. Native
record sort is stable insertion sort with fixed eight-word scratch. Wasm emits
the same bounded algorithm using the existing scalar host functions. Per-word
Wasm host mutations immediately update the live staged argument's handle; a
later failure therefore cleans up the current generation, and only the final
ordinary group commit transfers the owner to the returned value. Providers
retain their existing authority/handle validation and capacity limits.

## Private collection results

An explicit monomorphic two-case variant may carry one or two direct
`Vec<R>` fields across its declaration, with 1–8 fields per case and only
admitted direct scalars beside those vector fields. This direct outcome owns
its vectors; `own` transfers and `borrow` inspection use ordinary variant
patterns, authenticated conditional cleanup leaves and grouped call commits.
No recursive, generic, String/Bytes-bearing, third-vector, scalar-vector or
nested-record carrier is added by this profile. The independent source and HIR
classifiers live in `declared_type/collection_outcome.rs` and
`hir/collection_outcome.rs`.

Its private native field is the existing 40-byte Vec carrier; Core Wasm stores
one existing 8-byte host handle. The variant layout distinguishes the owned
Vec leaf from Copy fields and checks ordinary deterministic layout replay.
Source syntax, public ABIs, cleanup/graph schemas, host tags and limits remain
unchanged. The runtime never treats the outcome or vector field as Copy.
Copy variants containing a flat Copy record can also cross a private loop call
and be matched in that loop; this adds no owned loop-return or renewal rule.

The `copy_record_vec::outcome` gate fills the actual cardinalities of 256
six-field records and eight two-field identifier spans, then borrows, forwards,
consumes and drops both success and error results. It tests bounds failure
with both fields live, refusal of the second allocation, exact graph/source
round trips and forged ownership. Library `collection_outcome` gates separately
mutate declaration origin, source-versus-index field type and cached type facts,
and retain v27 refusal. These are authored qualification gates, not evidence of
a completed application or measured agent efficiency.

## Qualification and remaining application boundary

The authored `owned_data::copy_record_vec` gates cover a six-field scheduling
fragment with named fields, private helpers, loops, owner reuse, every operation,
all scalar field kinds, signed-zero ordering, mixed-width records, exact/over
capacity, bounds failures, allocation refusal, canonical/graph round trips and
hostile HIR. The physical corpus executes interpreter, native C11 O0/O2, and
validated Core Wasm under a strict moving-handle host; native allocation and
all host handles must settle after repeated invocations. The additional library
`copy_record_float_total_order_preserves_nan_payloads_at_internal_boundaries`
probe passes exact signed quiet/signaling NaN payloads, infinities and zeros
through private interpreter/native calls and test-only Wasm argument constants.
It verifies the full declared total order without changing public scalar
admission or permitting nonfinite source/HIR literals. These gates have not
been run by this lane.

Project v27 remains frozen: `stream_data_parameter_admitted` accepts only borrowed
scalar Vec helpers; `validate_stream_data_program` rejects authored nominal
closure declarations and record helper results. The authored [Stream Data Command v2](STREAM-DATA-COMMAND-V2.md) successor
reconstructs the admitted record closure and allows private Value record
parameters/results and borrowed/owned Vec transport, preserve `fn()->i64` roots,
provider limits and effects, and rejects other nominal closures. Its focused
execution and application qualification remain pending. Public ABI,
foreign/provider payload transport and source/cache authority remain unchanged.

Issue closure also requires the unchanged CLI 49, ShiftSim 15 and TeamDesk 912
acceptance gates, followed by at least five matched runs per language/arm using
the same prompts and model against strong idiomatic TypeScript baselines. Report
turns, net input, authored tokens, conditional accepted-task cost, wall time and
fixed harness context separately. Unavailable provider billing remains null.
No savings estimate follows from this implementation or its static source size.
