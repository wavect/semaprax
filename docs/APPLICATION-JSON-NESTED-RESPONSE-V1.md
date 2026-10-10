# Bounded nested application response JSON v1

Status: ordinary source implementation and owning regression definitions for
OPT-724; executable qualification is pending. No application acceptance or agent
cost/token advantage is established by this source batch.

The additive selector `bounded-nested-response.v1` is encode-only. It complements
[bounded nested request decoding](APPLICATION-JSON-NESTED-REQUEST-V1.md), while
preserving the existing collection response and request selectors exactly.
The library policy is `JsonCodecProfile::NestedResponse { max_string_bytes,
max_array_items }`. Both CLI bounds are required: canonical decimal String byte
bound 1..64 and array cardinality bound 1..256.

## Finite source shape

The root and all child records must have explicit identities, be monomorphic,
acyclic, invariant-free, and declared in the selected authenticated schema source.
Each record has 1..8 fields. Record depth is at most 8 and the expanded declaration
preorder has at most 64 field paths. Repeated child declarations count at each
path. Wire names are checked ASCII source identifiers; declaration order fixes
output order independently of input order, vector sorting or field names.

Leaves are `i64`, `u8`, `usize`, `bool`, or owned `string`. Up to **two expanded
Vec fields** may occur at distinct record paths. Elements are supported Copy
scalars or flat records with 1..8 supported scalar/String fields and at most two
String fields. Nested records inside Vec elements and additional collections,
Bytes, floats, resources, classes, generic records, variants, optional values and
recursive trees remain outside this selector. At least one String or Vec owner
must occur in the root closure.

Primitive `Vec<string>` is explicitly refused with `SPX-J180`: its current
copy-out operation allocates and it has no admitted nonallocating borrowed element
projection. Derivation must not silently clone or invent a borrowed primitive
String operation. String fields within flat record elements use the existing
scoped `vec_field` operation. Copy-record and scalar elements use existing checked
`vec_get` values. No Vec element family or runtime capacity changes.

The shared source descriptor selects the request's one-Vec/264-decoded-String
policy independently. Response inspection borrows already existing values and
has no decoded-owner allocation census. A response containing 256 two-String
rows plus a direct String remains eligible when its output fits; this is a
required witness, not a new global 512-String limit. The two-Vec, 64-path,
flat-row and cardinality bounds make the number of inspected leaves finite.
Actual ordinary layout, String/Vec storage, live-owner, allocation, fuel and
backend limits remain authoritative; this policy neither increases nor refunds
any of them. Up to two sibling vectors can therefore encode without broadening
request admission or imposing its materialization census on borrowed responses.

## Generated source and exact failure behavior

For `Report` with identity `response.report`, the generated ordinary APIs are:

```text
json_Report_nested_response_encoded_len(value: borrow Report) -> usize
json_Report_nested_response_encode(value: borrow Report, output_limit: usize)
    -> ReportJsonNestedResponseEncode
```

The result is `Encoded { text: string }` or `Refused { required: usize }`.
Their identities are `response.report.json.nested-response.encoded-len`,
`.encode`, and `.encode-result`. Child helper identities bind expanded field
ordinals in the same root namespace. UTF-8 helpers rewrite template namespaces
before inserting opaque authored names and identities.

The length operation allocates no String or vector, deep-clones no value, and
retains no escaping loan. It checks actual cardinalities and every decoded String's
UTF-8 byte bound, including NUL. The complete size includes all nested object
braces, literal ASCII keys/quotes/colons, array brackets, commas, exact signed or
unsigned decimal digits, boolean spelling, and JSON string escaping. Each child
size is checked before subtraction/addition; lazy failure guards prevent overflow
or underflow. A size exceeding the unchanged 131072-byte output envelope, an
invalid bounded String or an array exceeding the selected cardinality returns
`18446744073709551615usize`.

A value that passes preflight yields its exact encoded byte count. An output
limit below that count returns `Refused { required: exact_count }` before text
construction. An invalid/out-of-envelope value returns `Refused` with the sentinel
even if the caller supplies a larger limit. The encoder starts ordinary String
construction only after the entire recursive preflight passes. Success emits a
compact deterministic JSON object with declaration-order fields and array-order
elements. Escaped controls, quotes and backslashes retain the existing checked
UTF-8 renderer. No partial text is published by the outcome. Ordinary allocation
or call failures retain sticky runtime status and canonical cleanup; they are not
translated into an invented JSON allocation-error case.

Nested object helpers keep the same named root borrow and authenticate full
field paths inside their bodies. They do not pass a projected owning record as
an implicitly admitted borrowed nominal call argument. Vec helpers receive the
existing authenticated projected Vec borrow; indexed String-row helpers inspect
scoped fields from its same generation. Copy-row helpers receive ordinary Copy
values. Left-to-right evaluation, root-move blocking, source/HIR/loan replay,
backend layout and canonical cleanup remain ordinary language checks.

## Authority and qualification

The original Project is checked first; generated ordinary source is parsed,
canonicalized, and rebuilt under that same manifest, dependencies, capabilities
and profile. Names and schema digests confer no authority. The source imports
only existing `std.data.json.digits.i64_len`, `std.data.json.write.usize_len`,
and `std.data.json.utf8` scalar/navigation checks. Declare the bundled JSON
closure explicitly. No runtime capability or stdin permit is added. Exact
`verify_json_codec_source_with_profile` replay binds the selected policy and
original source; stale/modified artifacts refuse with `SPX-J180`.

Pure `owned-data-api.v1` Projects with empty authenticated exports admit the
internal corpus without a public nominal descriptor. A nested collection command
uses its independently checked v31 boundary; whole nested decoder outcomes
require the explicit v32 boundary. The selector does not widen public export or
older command profiles. A complete application must qualify its chosen command
route independently before publication or efficiency measurement.

Authored owning gates (not executed in this implementation lane):

- `--lib project::json_codec::nested_response::tests::` (4): deterministic source,
  canonical projection, preflight order, Copy/scoped reads, repeated/opaque
  identities, policy bounds, cycles, unsupported fields/third Vec and unchanged
  request allocation-census refusal.
- `--test project standard_library::application_json::nested_response::` (8):
  original-source/policy replay and modified artifact refusal; identical source
  on interpreter, native C11 O0/O2 and strict Core Wasm; Unicode/NUL/extrema,
  empty arrays, exact/one-short output, selected eight-row boundary, late dynamic
  String/array +1 rejection, stale authenticated schema claims, nested sibling
  scalar/Copy-record vectors, whole request-decode/response-encode composition,
  direct String plus 512 row String leaves, and escaped output-envelope refusal.

Runtime settlement and profile/loan replay gates remain required. Same-source
backend definitions, emitted artifacts and source-size reductions do not count
as execution, current-head application acceptance, billed cost or matched-agent
efficiency evidence.
