# Bounded application collection response source v1

This additive **encoder-only source selector** is
`bounded-collection-response.v1`, selected with a canonical decimal
`--max-string-bytes` from 1 through 64. The API profile is
`JsonCodecProfile::CollectionResponse { max_string_bytes }`. Source and runtime
qualification are pending the owning executable gate. The selector does not
grant v30 command admission, a public nominal ABI, or authority to install HIR.
It is a finite #724/#727 tranche, not a general recursive JSON codec.

The selected authenticated source must contain an explicitly identified,
monomorphic record with exactly two fields in either declaration order: one
direct `Vec<Row>` and one flat scalar `Metrics` record. Both child records must
be declared in that same source. Row has exactly one `string` and zero through
six `i64`, `u8`, `usize` or `bool` fields. Metrics has one through eight of those
scalar fields. Record and field identities are explicit, and no type parameters
or invariants are admitted. Existing source/name/schema/generated-byte limits
remain unchanged. Field names, type names and identities are arbitrary admitted
source names; a benchmark-specific name or generated origin grants no exemption.

For `Report { items: Vec<Item>, metrics: Metrics }`, generated APIs are:

```text
json_Report_collection_response_encoded_len(value: borrow Report) -> usize
json_Report_collection_response_encode(value: borrow Report,
                                       output_limit: usize)
    -> ReportJsonCollectionResponseEncode
```

Their identities are `<report-id>.json.collection-response.encoded-len` and
`<report-id>.json.collection-response.encode`. The ordinary affine outcome,
identified by `<report-id>.json.collection-response.encode-result`, is
`Encoded { text: string }` or `Refused { required: usize }`.

The encoder emits compact JSON in each record's **declaration order**. Vector
order is preserved, without sorting, repairing, deduplicating or reinterpreting
application data. Business sorting can use a different ordinary record and map
named fields to the response record before encoding. Integers retain their
complete exact ranges; booleans emit lowercase literals. Strings are validated
as UTF-8, including NUL and every Unicode scalar, then quote/backslash and
control bytes use deterministic JSON escapes. Other Unicode scalars remain
literal UTF-8. The bound is independently applied to every stored Row String's
UTF-8 byte length. Empty/repeated Strings and an empty vector are valid; domain
uniqueness and application numeric ranges remain the caller's responsibility.

Preflight checks the actual vector length (at most 256), stored String validity
and byte bound, and exact encoded UTF-8 byte length. Actual output over 131072
bytes, invalid Strings or excessive cardinality yield `usize::MAX`; encoding
returns `Refused { required: usize::MAX }`. A valid one-short caller output limit
returns the exact required byte count before response String construction.
The count/string maxima are not a guarantee that every possible combination of
long field names fits the output cap: such a value is refused explicitly.
No physical, fuel, borrowed-root or ownership bound is increased.

The helper borrows the actual Report owner for the complete call. Its ordinary
projected vector reads independently authenticate the full field path;
`vec_clone_at` produces one independent owning Row per read. It never clones the
Report or stored Vec. Row String length/quoting reads use the ordinary authenticated
`string_as_str(value.<field>)` projection from its named `borrow Row` parameter.
The full field path and live owner remain compiler proof obligations; no emitted
borrow-match alias, generated origin or implicit String clone grants authority.
This source form depends on the rooted projected String-view contract and its
owning executable gate. Metrics is Copy and passed by value. Preflight itself can
allocate temporary Row deep copies, so it is not allocation-free. Clone/String
allocation failures retain their ordinary checked status, argument staging,
group commit, sticky failure selection and partial-owner cleanup; they are not
converted into a JSON refusal or published partial output.

Derivation and replay rebuild the caller's unchanged Project from its exact
authenticated revision, selected stable type identity, policy and bound.
Canonical source round-trips and ordinary type/HIR/ownership/profile/backend
checks remain mandatory before exclusive CLI output publication. Missing child
types, stale output, a changed bound or an unsupported schema refuse as SPX-J180.
Generated source carries no filesystem/process/network authority and has no
special helper-origin rule. Nested collection runtime support belongs to the
independent ordinary compiler carrier contract; this selector cannot broaden
frozen v29/v30 command profiles.

Owning source gates are `project::json_codec::collection_response::tests::` and
the Project harness's
`standard_library::application_json::collection_response::`. They cover exact
source/policy replay, wrong schema refusal, declaration/wire order, Unicode/NUL,
all scalar boundaries, exact/one-short output, full cardinality, malformed
values and strict partial-clone cleanup, with the same admitted application
source on interpreter, native C11 O0/O2 and strict Core Wasm. No executable
acceptance, tokenizer saving or causal cost result is claimed before those
gates and a fresh source/binary-bound application qualification.
