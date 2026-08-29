# Frame payload Rust consumer

This package depends only on the generated safe owned-data SDK plus Serde for
reading the committed corpus. Build the Project-v8 Rust target, then run the
consumer:

```sh
cargo run --locked -- build --manifest-path examples/frame-payload-project/semaprax.toml --target rust -o examples/frame-payload-rust/generated-sdk
cp examples/frame-payload-project/corpus.json examples/frame-payload-rust/corpus.json
cargo run --manifest-path examples/frame-payload-rust/Cargo.toml
```

The generated SDK is local evidence and must not be committed. Its public
`spx_frame_dot_payload_hyphen_result` method is derived from the stable ID,
not the SEMAPRAX display name.
