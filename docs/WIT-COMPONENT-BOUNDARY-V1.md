# Private WIT boundary v1

Status: implemented behind `unstable-wit-component-harness`, disabled by
default. This is deterministic schema and adapter evidence, not a WebAssembly
Component Model binary/runtime claim.

`SPXWIT01` is an exact length-framed private bundle containing:

1. a versioned WIT 0.1 scalar evaluation interface;
2. canonical mapping JSON for `result<s64, status>`;
3. a strict JavaScript result/status adapter.

The bundle has a frozen SHA-256 known answer and an exact verifier. Tests reject
every single-byte mutation, every truncation, trailing bytes, and magic/version
confusion. Node executes both result branches and rejects hostile tags, scalar
types, status shapes, zero/overflow codes, empty/NUL/oversized UTF-8 domains,
and invalid class/retryability values. The WIT `status.domain` field is mapped
explicitly to `semaprax.status.v1.domain_id`; code and domain bounds match the
language status contract.

The next gate is a real component encoder/parser/runtime that composes this
mapping with generated Wasm and independently validates the WIT package.

## Nonclaims

This tranche does not emit or run a Component Model binary, import WIT,
implement resources, handles, futures, streams, capabilities, version
negotiation, multi-language component composition, or expose a public WIT API.
It does not change the existing core-Wasm owned ABI or `SPX-B104`.
