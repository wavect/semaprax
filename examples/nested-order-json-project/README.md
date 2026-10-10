# Finite nested order decoder

Source implementation and regression corpus; current-head execution is pending.
This example is a pure `owned-data-api.v1` Project, with no runtime capabilities,
command provider or public nominal export. Its independently authenticated
schema contains configuration metadata, one vector of order lines and a scalar
flag. It uses the opt-in `bounded-nested-request.v1` source-generation selector;
older codec selectors and command profiles remain unchanged.

Start from the checked bootstrap `src/app.spx`. Derive a new complete module,
then review and install it before selecting the authored consumer:

```sh
semaprax json-codec . --source src/schema.spx --type orders.request \
  --profile bounded-nested-request.v1 --max-string-bytes 16 \
  --max-array-items 8 --output src/schema.generated.spx
# Review the canonical checked replacement; these are explicit installation steps.
cp src/schema.generated.spx src/schema.spx
cp src/app.consumer.spx src/app.spx
semaprax check .
```

No generated helper body is committed or guessed. The actual generator checks
the original Project and its declared dependencies, then independently checks
the replacement before exclusive publication. It adds no capability and never
overwrites the destination. The caller's Project admission remains authoritative.

`json_OrderRequest_nested_decode(input: borrow Slice<u8>, input_limit: usize)`
returns `OrderRequestJsonNestedDecode::Ready { value: OrderRequest }` or
`Error { code: i64, offset: usize, field: i64 }`. In the consumer, `read_fixture`
decodes from lexical owned Bytes and returns only the owning outcome. Ready's
Strings and vector are independent of that retired input. The consumer transfers
the real root fields with `match own`; it never creates dummy default owners.
Repeated `vec_clone_at` reads produce independent owning line values. The
consumer checks exact Unicode/NUL metadata, numeric fields and row order before
returning 729; its failure result is zero.

Both bounds are canonical unsigned decimal: no signs, leading zeros, spaces or
non-ASCII digits. Each decoded String is at most 16 UTF-8 bytes, and the single
vector is at most eight items. Empty Strings and arrays are valid schema values;
no identifier uniqueness or nonempty-array application policy is inferred.
The input budget is separate and is checked before strict whole-document grammar.
No input, output, owner, fuel or physical target limit is raised.

The fixtures retain valid Unicode/NUL input, a truncated root, a duplicated
nested field, a 17-byte String and a ninth array element. Error offsets are raw
absolute byte offsets into the supplied immutable input, not normalized stream
positions. The owning regression fixes duplicate code2/offset31/field2,
String bound code6/offset26/field2, array bound code8/offset251/field4, and
structural EOF code1/offset93/field0. First-field-path ordinals are declaration
preorder: configuration1, label2, retry3, lines4, sku5, quantity6, urgent7.
Exact and one-short input limits, an exact 16-byte String and exactly eight
rows remain independent boundary witnesses.

The normative schema and error contract is
[APPLICATION-JSON-NESTED-REQUEST-V1](../../docs/APPLICATION-JSON-NESTED-REQUEST-V1.md).

The `project` harness's `standard_library::application_json::nested_request`
child is the owning compiler/three-backend gate. No compilation, runtime result,
model-authored token saving or benchmark improvement is claimed by this source
example. Native nested-outcome command transport is a separate explicit v32
route described by `docs/PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md`.
