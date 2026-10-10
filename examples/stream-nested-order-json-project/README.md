# Streamed finite nested orders

This is a source-only v32 native command example; current-head execution is
pending. Its ordinary schema, imports and four manifest grants are explicit.
No helper body is prewritten or guessed. The compiler derives a new complete
canonical module under the original Project authority:

```sh
semaprax json-codec . --source src/schema.spx --type orders.request \
  --profile bounded-stream-nested-request.v1 --max-string-bytes 16 \
  --max-array-items 8 --output src/schema.generated.spx
# Review and explicitly install the checked replacement and authored consumer.
cp src/schema.generated.spx src/schema.spx
cp src/app.command.spx src/app.spx
semaprax check .
semaprax build . --target native --output orders
./orders < fixtures/request.json
```

The expected request output is `3:3:true:2:6:6`: configuration label UTF-8 bytes,
retry, urgency, row count, total quantity and total SKU UTF-8 bytes. Cloned rows
are independent owning values. The stream wrapper holds normalized Bytes until
direct decoding returns a detached owning root, then retires those Bytes.

`json_OrderRequest_nested_stream_decode() -> OrderRequestJsonNestedDecode`
uses only the original `process.stdin.read` permit. It returns `Ready { value }`
or `Error { code, offset, field }`; the command transfers the real root fields
and reports typed failures with status 2. Stream grammar/capacity offsets are raw;
downstream schema offsets are normalized. For example, `  {"unknown":0}` reports
`error:4:1:0`, not a raw-offset claim.

Bounds 16/8 give an independently calculated maximal valid normalized spelling
of 1766 bytes: fully escaped keys, 16 escaped NUL bytes in every String, 8 rows,
u8 maximum 255,usize maximum 18446744073709551615 and `false`. Its output is
`16:18446744073709551615:false:8:2040:128`. Derivation rejects schemas whose
worst valid normalized spelling exceeds the existing 131072-byte owned buffer.
The direct selector remains unchanged and is available independently.

There is no raw-input or whitespace byte cap. The native regression retains
requests with 70–80 KiB of whitespace, strict malformed/duplicate/String/array
bound faults and invalid over-buffer content with a later grammar fault.
Semantic-buffer overflow does not stop grammar validation or publish partial
owners. Missing original stdin permits and unsupported normalized envelopes
refuse derivation without changing source.

The owning Project child tests unmodified native O0/O2 output separately from
an explicitly test-instrumented provider. It requires exactly one matched fread
site, records source/HIR/original-C/instrumented-C hashes and asserts actual
returned 4096-byte chunk inventories at every split of a representative request
containing raw UTF-8, NUL escape and a surrogate pair. Pipe writes alone are not
boundary evidence. Native command transport is not claimed as Wasm transport;
the existing pure nested decoder fixtures retain three-backend parity.

Contracts: [nested codec](../../docs/APPLICATION-JSON-NESTED-REQUEST-V1.md) and
[v32 command profile](../../docs/PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md).
No execution, model-token saving or benchmark cost improvement is claimed here.
