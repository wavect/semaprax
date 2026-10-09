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
fields and arrays are outside v1. #723's flat Copy record Vec carrier is a
separate composition step; this generator does not broaden collection ownership.

Declare the exact bundled dependencies `std.data.json.scan`,
`std.data.json.token`, `std.data.json.digits` and `std.data.json.write`.
The implementation uses the existing strict whole-document scanner, decoded
key equality, exact token conversion and decimal length helpers. Legacy scanner
policies and identities remain unchanged. It neither adds dependencies behind
the caller's back nor acquires a runtime capability.

The owning gate uses private `owned-data-api.v1` Projects with empty web
exports. Interpreter, native C11 O0/O2 and strict Core Wasm consume the same
checked source. Public nominal exports and SourceCommand v27's scalar helper
boundary are unchanged. Native streaming command composition needs an explicit
successor profile and additional execution evidence. In particular, this slice
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
Failure offset is an absolute byte offset in the immutable input; EOF equals
input length. Global errors use field zero. The codes are:

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

`PatientJsonEncode` is affine: `Encoded { text: String }` or
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

Generated source bytes are compiler output. They must be reported separately
from model-authored source bytes/tokens in any efficiency comparison. No token
savings, current-head acceptance, broader application profile or cost advantage
is established before the required fresh matched campaign and full #724 gates.
