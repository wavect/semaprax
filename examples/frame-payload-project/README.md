# Frame payload validation product

Status: authored, unrun; not hosted promotion or a published SDK.

Audience: contributors and cross-target conformance reviewers.

This Project-v8 example decodes `SPX1`, a four-byte big-endian payload length,
and raw payload bytes. The committed `corpus.json` covers nine cases, including
empty/truncated input, bad magic, empty/text/NUL/invalid-UTF-8 payloads, the
65,528-byte maximum payload, and a declared-length mismatch. The direct
`frame.payload` API is called only for valid frames; malformed frames exercise
the Option and Result APIs instead.

The [web consumer](../frame-payload-web/README.md) and
[safe Rust consumer](../frame-payload-rust/README.md) run that identical corpus.
The Rust target requires the unpublished `semaprax-full` toolchain host.

The focused integration gate is:

```sh
cargo test --locked -p semaprax --test frame_payload_product_v1
```

Its actual Project-route fixture builds both packages before and after one
display-only `payload_result` rename. It reopens the exact npm inventory and
compares it with the retained Project's replayed build, independently replays
the shared descriptor against that subject, and checks the Rust package's
seven-file inventory, descriptor, artifact hashes, provider-source binding and
exact canonical manifest. Native manifest checks are test-specific; there is
no claim of a public reopened-package verifier or archive provenance proof.
Cross-paired old/new descriptors must reject, and only the three descriptor
revision/graph bindings may change.

Both real linked subjects also run the original interpreter and native O0/O2
corpus oracles. A separate raw-Wasm ABI consumer checks the exact imports and
exports, normalized statuses, active tags/bytes/errors, real owner mint and
copy-out settlement counts, empty settlement, and pre-import result-pointer
rejection. It reuses the unchanged production arena/input/core templates; it
is not an independent arena implementation or a full internal destruction
trace. The generated npm and Rust consumers remain separate executions.

An additional native O0/O2 lane wraps the exact provider with a calibrated
allocator observer. It checks physical payload allocation/free calls and live
pointers as well as handle settlement, including empty owners, inactive variant
branches, and stale drops. It preserves the plain-provider runs and the same
nine-case corpus for both Project subjects. The new checks are authored but
unrun, and do not establish sanitizer or allocation-failure recovery evidence.

Clang, Node, and the full native SDK toolchain must already be provisioned.
Missing tools fail rather than skip. TypeScript and Chromium have separate
explicitly selected provisioned gates documented by the web consumer.

This is not multi-browser evidence or permission to widen any earlier profile.
