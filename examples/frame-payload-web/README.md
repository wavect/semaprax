# Frame payload npm consumer

This consumer uses only the generated owned-data npm package. From the repository
root, build the canonical Project-v8 manifest and execute the shared hostile
corpus:

```sh
cargo run --locked -- build --manifest-path examples/frame-payload-project/semaprax.toml --target npm -o examples/frame-payload-web/generated
cp examples/frame-payload-project/corpus.json examples/frame-payload-web/corpus.json
node examples/frame-payload-web/consumer.mjs
```

The generated directory is evidence output and must not be committed. The
TypeScript consumer pins stable-ID access through
`runtime.functions["frame.payload-result"]`; a source display rename cannot
change that access path.
