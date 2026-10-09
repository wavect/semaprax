# Owned JSON command project

This Project v30 example derives a stream-aware request codec from an authored
`Request { locations: Vec<string>, stock: Vec<Stock> }` schema. The native
command normalizes stdin, decodes independent owned strings and records, then
lets the normalized `Bytes` owner retire before sorting, updating, and encoding.
It decrements the first positive stock quantity and writes one JSON document.
Malformed input returns status 2 without output.

This example follows the current ASCII identifier policy: up to eight unique
1–16 character identifiers in `locations`, up to 256 `Stock` rows, and a
nonempty row list requires at least one location. Identifiers use ASCII
letters, digits, `_`, or `-`. This does not claim arbitrary Unicode request
fields, recursive records, nullable values, or a general owned JSON tree.

The project starts with valid schema and command stubs. From the repository
root, copy the directory and bootstrap the derived module:

```sh
cp -R examples/owned-json-command-project /tmp/owned-json-command-project
cd /tmp/owned-json-command-project
semaprax check .
semaprax json-codec . --source src/schema.spx --type warehouse.request \
  --output src/schema.generated.spx --profile stream-owned-request.v1
cp src/schema.generated.spx src/schema.spx
cp src/app.command.spx src/app.spx
semaprax check .
semaprax test .
semaprax build . --target native -o owned-json
./owned-json < fixtures/request.json
```

The expected successful output is:

```json
{"locations":["NORTH","SOUTH"],"stock":[{"sku":"CRATE-1","quantity":7,"available":false},{"sku":"CRATE-2","quantity":4,"available":true}]}
```

The generated output path must be new. Review the generated schema module and
then use the two `cp` commands to install that complete module and the
separately authored consumer by ordinary filesystem operations.
`fixtures/malformed.json` exercises the typed error branch; the process exits
with status 2 and writes no stdout.

The source example and focused native gate are added, but the grouped
current-head check is pending. Do not treat this README as execution evidence.
