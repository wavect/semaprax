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

`corpus-runner.mjs` is the shared browser-neutral full-corpus runner used by
`consumer.mjs` and the actual `index.html`/`browser.mjs` browser entry. Node
evidence is not browser evidence. The separate provisioned
[Chromium gate](../../platform-tests/frame-payload-browser-v1/README.md)
executes both baseline and renamed packages without installing tooling.

Select strict TypeScript evidence explicitly with an already provisioned
TypeScript 5.8.3 executable (the repository-pinned tool is reused):

```sh
TSC="$PWD/platform-tests/wasm-scalar-browser-v1/node_modules/.bin/tsc" cargo test --locked -p semaprax --test frame_payload_product_v1 consumer_acceptance::strict_typescript_accepts_both_display_names_and_rejects_wrong_types -- --ignored --exact
```

On Windows, set `TSC` to the provisioned `tsc.cmd`. The selected gate fails
if the compiler is absent or has the wrong version; it does not download or
silently skip. It checks the real consumer in strict mode against both
generated packages and requires specific diagnostics for invalid argument and
unchecked Result-union access. These gates are authored, unrun, and are not
selected by any newly modified hosted workflow.
