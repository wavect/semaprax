# Checked-source application JSON codecs v1

Status: first source implementation for #724; focused execution and repository
gates pending. This does not close the broader application JSON codec issue.

`semaprax json-codec <project> --source <module-path> --type <record-id>
--output <new-file>` derives a complete canonical replacement for one existing
module. It checks the original Project, derives from that module's authenticated
record declaration, and rebuilds the complete Project with the replacement in
memory. The existing manifest profile, capabilities, exports and dependencies
remain authoritative. Only a fully checked replacement is returned. The CLI
rechecks held source bytes before publication, creates a private file beside the
explicit destination, and publishes through an atomic exclusive hard link.
Existing destinations, including a racing creator, are refused; original source
and manifest files are never overwritten. Install the reviewed replacement
through the ordinary source or semantic transaction workflow.

This is executable source generation, not the Agent proposal schema decoder,
Rust interop Serde, an owning JSON tree, or a schema digest granting HIR authority.
Helper names have no compiler exemption. Canonical parsing, resolution,
independent HIR/loan/cleanup validation, graph construction, profile admission
and cache/source replay are the same boundaries used by authored code. The
library `project::verify_json_codec_source` rederives exact bytes when a caller
claims a particular artifact was generated from a particular immutable revision.
Editing generated source removes that derivation claim; ordinary compiler
checking still applies.

The additive `bounded-collection-response.v1` selector is encode-only. It accepts
one `Vec<Row>` plus one flat scalar metrics record and requires
`--max-string-bytes 1..64`. Its complete owning signatures, declaration-order
wire contract, preflight refusals and executable gates live in
[Bounded collection response v1](APPLICATION-JSON-COLLECTION-RESPONSE-V1.md).
Nested collection runtime admission is independent; a native command selects
the explicit [Project v31 profile](PROJECT-V31-COLLECTION-RECORD-COMMAND-V1.md).
The source implementation is integrated; current-head qualification is pending.

## Admitted data contract

The first slice admits a monomorphic flat record with 1 through 8 directly
declared `i64`, `u8`, `usize` or `bool` fields. The record and every field must
have explicit persistent identities. Field display names are ASCII identifiers
of at most 64 bytes; they are the JSON wire names. Record display names have the
same identifier and length bound. Declaration order is output
order, and a one-based declaration ordinal identifies a field in an error.
Input member order is unrestricted. Record IDs use ASCII letters, digits,
dot, underscore, hyphen or colon and are at most 80 bytes so all
derived identities stay within the existing 128-byte identity contract.

The selected authored schema module is at most 65,536 bytes. Generated helper
source is at most 131,072 bytes; the complete canonical replacement is bounded
by the sum. These are compiler artifact bounds, not raw JSON input limits.
Record invariants refuse derivation: this generator cannot replace explicit
application validation with an unchecked schema claim. Classes, resources,
generic records, nested fields, floats, chars, owned strings, nullable/optional
fields and arrays are outside the default scalar profile. Additional closed view
profiles below admit bounded identifier/array schemas without constructing owned
String records. #723's flat Copy record Vec carrier is a
separate composition step; collection ownership remains independently checked by the compiler.

Declare the exact bundled dependencies `std.data.json.scan`,
`std.data.json.token`, `std.data.json.digits` and `std.data.json.write`.
The implementation uses the existing strict whole-document scanner, decoded
key equality, exact token conversion and decimal length helpers. Legacy scanner
policies and identities remain unchanged. It neither adds dependencies behind
the caller's back nor acquires a runtime capability.

The owning gate uses private `owned-data-api.v1` Projects with empty web
exports. Interpreter, native C11 O0/O2 and strict Core Wasm consume the same
checked source. Public nominal exports and SourceCommand v27's scalar helper
boundary are unchanged. An additional owning fixture selects the explicit
`language-command-io.stream-data.v2` successor: a native process reads one
bounded stdin chunk, decodes a record, stores and reads it through `Vec<Patient>`,
computes a decision, and writes the encoded result to stdout. Malformed inputs
return status 2 without partial output. This composition gate is pending; the
command fixture refuses additional chunks rather than claiming an incremental
whole-stream codec. That original fixture alone
does not establish ShiftSim's raw input above 65,536 bytes, nested requests,
owned string patients or its complete fifteen-obligation acceptance contract.

## Generated API and ordinary failure outcomes

For a record named `Patient` with identity `application.patient`, the ordinary
source functions are:

```text
json_Patient_decode(input: borrow Slice<u8>, input_limit: usize)
    -> PatientJsonDecode
json_Patient_encoded_len(value: Patient) -> usize
json_Patient_encode(value: Patient, output_limit: usize)
    -> PatientJsonEncode
```

`PatientJsonDecode` is Copy: `Decoded { value: Patient }` or
`Error { code: i64, offset: usize, field: i64 }`. No Patient is constructed before
complete grammar and schema validation. Required fields have no defaults.
Failure offset is an absolute byte offset in the immutable input; a selected
structural EOF equals input length. Truncated literals retain the strict
scanner's token-start selection. Global errors use field zero. The codes are:

| Code | Meaning and selection |
| --- | --- |
| 1 | Strict grammar, UTF-8, escape, surrogate or depth rejection, at the scanner's selected offset |
| 2 | A repeated known field, including decoded-equivalent escaped names, at the second opening quote |
| 3 | Missing required field; first absent field in declaration order, offset at EOF |
| 4 | Unknown member; opening quote and field zero |
| 5 | Wrong root or field value type; value start |
| 6 | Integer range or non-integer number lexeme; value start |
| 7 | Input length exceeds the caller's input budget; offset and field zero, before scanning |

Budget refusal precedes scanning. Complete strict grammar validation precedes
schema checks. Subsequent schema rejection follows member order; missing fields
are selected only after all supplied members pass. Unknown members reject at
their first occurrence, so repeated unknown members cannot supersede that
earlier rejection. Known duplicate keys use decoded Unicode equality. There is
no hidden coercion, rounding, exponent acceptance or decimal-fraction acceptance.
Signed `i64` admits its complete exact range and `-0`. `u8` admits 0 through 255.
Portable `usize` admits 0 through 2^64-1 and rejects every negative spelling,
including `-0`. Booleans accept only JSON `true` and `false`; null is a type error.

The caller's input budget is a byte admission bound, not a fabricated count of
CPU instructions. Existing execution fuel remains independently authoritative.
Strict validation has the existing 32-container depth bound; this generated
flat schema retains at most eight seen flags and scalar values. Schema key
comparison does not allocate a key vector or decoded tree. Byte admission is
checked before invoking the scanner; no new universal raw-input cap is imposed.
Actual borrowed-root and target memory limits still apply.

`PatientJsonEncode` is affine: `Encoded { text: string }` or
`Refused { required: usize }`. The exact size is computed without allocating
output. A short output limit returns Refused before any String construction.
Success emits one compact object, in declaration order, with ASCII field names
quoted, canonical decimal integers and lowercase booleans, and no whitespace.
Output text is published only after all ordinary owned calls complete. Argument
staging, ownership group commits, intermediate cleanup and sticky runtime failure
selection retain their existing language meaning. Allocator/contract failures
retain that runtime status; this first tranche does not turn physical allocator
failure into a JSON Error case or claim per-allocation fault-injection evidence.

## Owning gates

`project::json_codec::tests::` owns shape refusal, canonical source and pure
ordinary AST emission. The `project` integration harness's
`standard_library::application_json::` owns exact derivation replay/mutation
refusal, dependency preservation, a second configuration shape, and the actual
same-source scalar-record decode/decision/encode corpus on interpreter, C11
O0/O2 and repeated strict Core Wasm. The corpus fixes grammar/duplicate/missing/
unknown/type/range offsets, Unicode/NUL rejection policy, signed/unsigned limits
and exact/one-short capacities. `cli::json_codec::tests::` owns closed command
grammar and no-overwrite publication.
`standard_library::application_json::stream::` owns the selected v29 native
process composition, repeated canonical output and typed failure publication.

Generated source bytes are compiler output. They must be reported separately
from model-authored source bytes/tokens in any efficiency comparison. No token
savings, current-head acceptance, broader application profile or cost advantage
is established before the required fresh matched campaign and full #724 gates.

## UTF-8 request storage bounds

The selected `Utf8OwnedRequest { max_string_bytes }` policy admits plain String
values with an explicit decoded UTF-8 byte limit from 1 through 64, up to eight
first-array values and 256 rows. Its schema admission checks the canonical
**output** maximum: authored identifier field names emit literally, while each
decoded String byte can require six JSON bytes for a control escape. It does
not charge six escaped spelling bytes per field-name byte as canonical output.
Empty and repeated values remain valid under this generic policy.

This output bound is separate from actual input storage. A direct decoder
borrows the caller's immutable bytes under the ordinary target's byte limits.
The streaming normalizer keeps its unchanged 131072-byte physical buffer.
Equivalent escaped key/value spellings can exceed that buffer even when their
decoded values would fit the schema: the complete raw grammar is still checked,
and a grammatically valid oversized normalized input returns code 9 at the
first unstored raw byte. It never truncates input or converts a later grammar
fault into a capacity error. Exact-capacity, one-more-byte and late malformed
tail cases belong to the native stream harness. The existing full 264-by-64-byte
raw and escaped String witness remains an independent three-backend obligation;
schema arithmetic alone establishes no fuel or allocator acceptance claim.

## Owned identifier request successor

The additive `OwnedRequest` (`owned-request.v1`) and `StreamOwnedRequest`
(`stream-owned-request.v1`) selectors materialize actual `Vec<string>` and
`Vec<Row>` values from the identifier request schema below. This is a new source
tranche with executable qualification pending. It removes the application-side
span-to-String and field-to-record conversion. It retains the explicit ASCII
identifier policy, eight/256 array bounds, schema-derived storage bound,
uniqueness rule and required-server rule; it does not claim arbitrary Unicode
String fields, recursive records, optional fields or a general owned JSON tree.
Root/row names and wire field names are arbitrary checked source identifiers.
The single String field may occur anywhere among the row's scalar fields.

For `Request { servers: Vec<string>, patients: Vec<Patient> }`, the APIs are:

```text
json_Request_owned_decode(input: borrow Slice<u8>) -> RequestJsonOwnedDecode
json_Request_owned_encoded_len(words: borrow Vec<string>, rows: borrow Vec<Patient>) -> usize
json_Request_owned_encode(words: borrow Vec<string>, rows: borrow Vec<Patient>, output_limit: usize)
    -> PatientJsonViewEncode
```

Success is an ordinary affine `Decoded` case with the original root field names
and owning types; errors retain the exact `Error { code, offset, field }` values
from request-view validation. No owned String or authored row is materialized
until the complete grammar, shape, values, cardinalities and uniqueness checks
succeed. The checked view collections are temporary implementation values and
are settled after materialization. Each String/row construction and push uses
ordinary left-to-right staging and grouped ownership commit. A later runtime
allocation failure settles both partial owning collections and their staged
element under the existing sticky runtime status; it is not malformed JSON.
The returned owning values retain no offsets or loans into the supplied bytes.
The caller may settle the input before sorting, updating or encoding them.

The encoder rechecks identifier policy, cardinality and uniqueness on the actual
owned values. The exact encoded size uses original declaration order and wire
names. Invalid values return `Refused { required: usize::MAX }`; a short output
budget returns the exact required size before JSON output construction. Explicit
`vec_clone_at` is the admitted nonescaping read model, so validation itself can
allocate temporary deep copies; this is not an allocation-free preflight claim.
Duplicate checking is bounded pairwise comparison. Its runtime cost remains to
be measured. Actual String allocator failures retain the existing runtime
contract, and exhaustive allocator fault injection remains an open #724 gate.

The stream selector also emits the existing `json_Request_stream_normalize`.
Its original `process.stdin.read` module permit and manifest grant remain
required. Call `owned_decode` on the Ready slice, then settle Ready's Bytes as
soon as decoding returns. Raw stream errors and normalized schema errors retain
their separate documented offset domains; no generated wrapper conflates them.
The original Project is rebuilt unchanged. Native command composition requires
v30; v29 and older command profiles refuse this owning runtime closure. The
original request declaration remains a logical schema, not an admitted runtime
record containing nested vectors. The direct two-Vec success outcome has its
own independent source/HIR shape proof and ordinary variant layout/cleanup.

Owning source gates are `project::json_codec::owned::tests::` and
`project standard_library::application_json::owned::`: deterministic checked
derivation/replay, input-owner retirement, actual sort/update/encode, exact and
short output limits, typed malformed cases, an unrelated catalog with a moved
String field, interpreter/C11 O0/O2/strict Wasm parity, allocation refusal at each
partial materialization push and encoder clone, plus a v30 native stream above
65 KiB raw whitespace. These are staged regressions, not executed evidence or
completion of full ShiftSim, arbitrary nested codecs or agent efficiency.

## Streamed UTF-8 owned request successor

`utf8-owned-request.v1` decodes bounded Unicode String values from a direct
borrowed slice. Its additive stream counterpart,
`stream-utf8-owned-request.v1`, composes the same owned request codec with the
existing bounded stdin normalizer. Select it with canonical
`--max-string-bytes N` from 1 through 64; the limit applies separately to each
decoded String in the first array and each row. It keeps the UTF-8 profile's
0..8 first-array and 0..256 row bounds and accepts empty or repeated values,
including Unicode and NUL, and a nonempty row array when the first array is
empty. It does not widen the 65,536-byte foreign borrowed-root limit or the
131,072-byte owned-buffer capacity.

For `Request { words: Vec<string>, rows: Vec<Row> }`, the generated calls are:

```text
json_Request_stream_normalize() -> RequestJsonStreamInput
json_Request_utf8_owned_decode(input: borrow Slice<u8>) -> RequestJsonUtf8OwnedDecode
json_Request_utf8_owned_encoded_len(words: borrow Vec<string>, rows: borrow Vec<Row>) -> usize
json_Request_utf8_owned_encode(words: borrow Vec<string>, rows: borrow Vec<Row>, output_limit: usize) -> RowJsonViewEncode
```

The source schema must already declare `process.stdin.read`; codec derivation
does not add a permit or manifest capability, and the v30 command manifest must
grant that capability. The generated
`json_<Request>_stream_normalize` returns
`RequestJsonStreamInput::Ready { bytes, length }` or
`RequestJsonStreamInput::Error { code, offset, field }`. Pass only the Ready
slice to `json_<Request>_utf8_owned_decode`. The result owns its Strings and
rows, so the caller can settle Ready's Bytes after decoding and then make its
application decision and call the UTF-8 owned encoder.

Offset domains remain separate: normalization errors identify raw stdin bytes;
request/schema errors after Ready identify normalized bytes. The stream profile
requires the v30 native owned-data command route. Its public example and grouped
current-head qualification are pending; source generation alone does not prove
execution or full #724 completion.

## Identifier, request and incremental stream views

The additional closed CLI selectors are `--profile identifier-views.v1`,
`--profile request-views.v1` and `--profile stream-request-views.v1`. These are
source implementations with their owning combined gates pending. View profiles
use `std.data.json.query` from the declared scanner's authenticated bundled
dependency closure; an explicit query dependency is also accepted. They do not
change the default scalar codec. A selected identifier record has exactly one
`string` field and up to six scalar fields; its runtime view replaces that
String with two `usize` token bounds. Request views derive from an explicit
record whose first field is `Vec<string>` and whose second is `Vec<IdentifierRecord>`.
The faithful logical schema is checked by ordinary declaration-only collection
rules; those String collections cannot be constructed or used at runtime.
The generated runtime carriers are flat Copy views, `Vec<View>` and an ordinary
owned two-collection outcome. No schema or helper identity grants an exemption.

The request decoder admits zero through eight server identifiers and zero
through 256 records. Identifiers decode to 1 through 16 ASCII letters, digits,
underscore or hyphen; escaped-equivalent spellings compare as the same value.
Identifiers are unique within each array. A nonempty record array requires at
least one server. Extra fields, duplicate fields, missing fields and wrong
field types use the existing codes. Code 8 rejects the first array item beyond
its cardinality bound; code 10 rejects the second decoded-equivalent identifier;
code 11 rejects the nonempty-record/no-server combination at EOF. Array errors
identify the original item token offset and the root field ordinal; scalar
record errors identify the nested field ordinal. Numeric representations retain
the full scalar range. Application ranges, such as ShiftSim service 1..100000,
remain explicit application checks rather than silently narrowing `i64`.

The functions `json_Patient_view_decode`, `json_Patient_view_array_decode`,
`json_Request_request_decode` and corresponding encode/encoded-length helpers
are ordinary source. The request success carries the two owned view collections;
encoders borrow them and the immutable caller-supplied byte source. Bounds are
plain values carrying no authority: encoding rechecks each token's bounds,
complete string token, decoded length and ASCII policy against that supplied
source before constructing output. Request/array preflight also rejects
decoded-equivalent duplicate identifiers and nonempty patients without servers.
Callers retain the source for every view use.
Invalid spans return `Refused { required: usize::MAX }`; exact and one-short
output budgets preserve preflight before allocation. Canonical output decodes
escapes and emits compact ASCII identifiers in declaration order.

The stream selector additionally emits `json_Request_stream_normalize`, using
only the original schema module's explicit `process.stdin.read` permit. It
returns ordinary `Ready { bytes: Bytes, length: usize }` or
`Error { code: i64, offset: usize, field: i64 }`. It reads the entire provider
stream, carries lexical and structural state across chunks, validates raw UTF-8
and escapes, and removes whitespace only outside strings. It uses the existing
32-container stack and 131072-byte owned buffer. A checked, schema-derived bound
covers every admissible escaped key, identifier, scalar and maximum array
cardinality; schemas exceeding that physical bound refuse derivation. Raw
whitespace has no additional byte cap. A pending buffer refusal does not stop
validation: a later structural error wins, otherwise the earlier escape/UTF-8
fault wins, and only a grammatically valid oversized normalized document returns
code 9. Incomplete literals identify their token start, matching the strict
scanner; structural EOF uses the raw input length. Existing execution fuel,
provider failures, ownership and sticky allocation failure remain authoritative.

Offsets deliberately have two domains. `StreamInput::Error.offset` identifies
raw provider input. A request/schema error after `Ready` identifies the immutable
normalized bytes supplied to that decoder. For raw `  {"unknown":0}`, normalization
succeeds and request decoding returns code 4 at normalized offset 1, not raw
offset 3. Direct-input decoders continue reporting their supplied input's absolute
byte offsets. A wrapper must label these domains accurately. The Ready byte
owner remains live through view decoding, domain checks and encoding.

`standard_library::application_json::views::` owns same-source view codecs on
interpreter, native C11 O0/O2 and strict Core Wasm, exact/one-short output and
forged spans. `standard_library::application_json::stream_native::` owns a real
v29 stdin request pipeline, all lexical split positions, more than 65 KiB raw
whitespace, 256 records plus eight servers with escaped keys/identifiers, and
late grammar precedence after storage overflow. Its boundary probe emits C from
the same retained HIR and observes exactly one provider read site without
changing returned bytes/counts/status; assertions check actual read lengths.
That explicitly instrumented test executable is distinct from the unchanged
native acceptance executable. Source/HIR-graph/C hashes bind the probe.

General nested objects, nullable values and runtime materialization of owned
String records remain outside these profiles. Full ShiftSim scheduling and all
fifteen unchanged obligations, another complete application and physical
allocation-failure coverage are still required before closing #724. These
source tranches establish no executed current-head acceptance or token gain.
