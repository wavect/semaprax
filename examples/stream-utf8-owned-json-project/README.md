# Streamed UTF-8 owned JSON request

This v30 native command derives a bounded UTF-8 request codec from the authored
`Request { labels: Vec<string>, rows: Vec<Row> }` schema. It normalizes stdin,
decodes the `Ready` bytes into independent owned strings and records, updates the
first row, then encodes the result. Empty and repeated strings, Unicode, and
NUL are valid values; this example uses an empty label array with nonempty rows.

The stream normalizer reports raw-input offsets. Once it returns `Ready`, JSON
request and schema errors report offsets in the normalized bytes. Both error
branches return status 2 and write no stdout. Each decoded string is bounded
individually by 64 UTF-8 bytes. The 65,536-byte foreign borrowed-root limit and
131,072-byte owned-buffer capacity are unchanged; raw whitespace is handled by
the stream normalizer without a separate raw-byte cap.

From the repository root, copy the project and derive the stream codec:

```sh
cp -R examples/stream-utf8-owned-json-project /tmp/stream-utf8-owned-json-project
cd /tmp/stream-utf8-owned-json-project
semaprax check .
semaprax json-codec . --source src/schema.spx --type catalog.request \
  --output src/schema.generated.spx --profile stream-utf8-owned-request.v1 \
  --max-string-bytes 64
cp src/schema.generated.spx src/schema.spx
cp src/app.command.spx src/app.spx
semaprax check .
semaprax build . --target native -o stream-utf8-owned-json
./stream-utf8-owned-json < fixtures/request.json
```

The fixture produces:

```json
{"labels":["","\u0000","é","é"],"rows":[{"number":5,"text":"é\u0000😀"},{"number":4,"text":""},{"number":4,"text":""}]}
```

The output path must be new. Review the generated module, then install it and
the separately authored command through ordinary filesystem operations. The
focused owning example gate exercises the command with ordinary input, more
than 65,536 bytes of surrounding stream whitespace, malformed JSON, and a
well-formed value with the wrong field type. Its grouped current-head check is
pending.
