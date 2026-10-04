# RI-13 Wasm target classification

This local current-head run used the `semaprax` compiler built from clean
`c85b847a2e338abe66fa8edc1d99ab382f05a8b8` with locked offline Cargo.
The receipt hashes that compiler and the three Project manifests. It records
the actual CLI commands, exit codes, diagnostic codes, stdout/stderr hashes,
and whether the requested output path appeared.

| Project | Result | Scope |
| --- | --- | --- |
| M1 Regex/Url | Refused (`SPX-B107`) | Indexed Native Rust imports are not a Wasm route. |
| M2 record/iterator | Scalar Wasm build succeeded | This is the ordinary scalar Project only; the separately generated Rust Serde/Iterator adapters are not included. |
| M3 local HTTP | Refused (`SPX-W120`) | The source-local Future/host HTTP route is not a Wasm route. |

`raw/` contains all six command streams and the M2 scalar Wasm package.
`artifact-digests.json` binds every retained file by SHA-256, including the
`app.wasm` bytes. This is target classification evidence, not a cross-target
runtime or generated Rust interop conformance claim.
