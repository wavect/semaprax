# JSON codec publication seal v1

Audience: compiler contributors and reviewers.

Status: local compiler-side qualification on macOS arm64 for
[OPT869](https://github.com/wavect/semaprax/issues/869). Runner execution
authority and the OPT606 campaign remain pending.

## Explicit selection

```text
semaprax json-codec <project> --source <declared-module-path> --type <record-id> --output <new-receipt.json> [ordinary profile and bounds] --retained-receipt --receipt-stdout-seal <nonce>
```

`--receipt-stdout-seal` and its nonce occur exactly once. The nonce is 1..64 bytes, each an ASCII letter, digit, hyphen or underscore, and must not start with `--` (a flag-like missing/invalid value). A single leading hyphen and interior hyphens are valid. It is an opaque correlation value, not a secret or authorization. Empty, overlong, Unicode, whitespace, duplicate and missing values refuse with usage status2 before Project authority. The sealed form requires exactly one `--retained-receipt`; ordinary source, plan and explain forms cannot select it. All ordinary selectors and bounds retain their existing validation and diagnostic order. Absent the new flag, original single-codec, plan, explain and retained-receipt behavior is unchanged.

## Exact bytes and publication order

The compiler runs the same ordinary authenticated Project receipt receiver. It serializes the same complete receipt with the same terminal LF. The seal is derived from that exact in-memory publication buffer, before the unchanged exclusive no-overwrite publisher is called. It does not reopen an output path for hashing or generate a second filesystem artifact. No seal is emitted if ordinary compilation/derivation, encoding, private write/sync or exclusive publication fails.

Only after exclusive publication succeeds does the compiler write and flush one compact JSON record, followed by exactly one LF, to stdout. It has exactly these fields in this serialization order:

```json
{"schema":"semaprax.json-codec-publication-seal.v1","nonce":"runner-1","receipt_bytes":123,"receipt_sha256":"sha256:<64 lowercase hex digits>"}
```

The example byte count/digest are placeholders, not a known answer. `receipt_bytes` is the entire published JSON-plus-LF buffer length; `receipt_sha256` covers every one of those bytes. The record including its LF is at most512 bytes. The nonce does not affect receipt contents or derived sources. Ordinary diagnostic output retains its existing channel.

A stdout write or flush failure after publication returns the ordinary `SPX-J181` I/O diagnostic with the message `codec receipt was published, but cannot write stdout seal: <I/O error>`. The published receipt remains. Stdout may contain no bytes, a prefix, or a complete record whose flush failed. Consumers must require successful command completion as well as a complete valid record. No rollback or successful seal is asserted on error; a retry to the same artifact refuses rather than overwriting it.

## Trust boundary and accounting

The record does not authenticate its own producer. A host must separately establish the actual executing binary, trusted stdout/status origin, exact invocation association, and effective sandbox/environment continuity before using it as execution evidence. In an agent-writable mailbox it remains untrusted observation. A future qualified runner can correlate its expected nonce, compare the complete record's length/SHA with its retained no-follow output snapshot, and refuse on drift. A nonce alone supplies no authority, and a stdout seal does not certify the mutable path remains unchanged after publication.

Project authority, retained originals, two actual same-process derivations, null compiler binary identity and `independent_execution = false` remain unchanged. This adds no filesystem, process, network, environment-read or execution capability. There is no independent-process rerun, authored/generated token subtraction, billing, complete OPT606 measurement or trusted executable claim. Existing host authentication/completeness flags remain false until their distinct prerequisites are qualified.

## Owning checks

On macOS arm64, six binary-unit tests and two Project integration tests passed
using the local compiler at `c28cc4432`. The binary tests exercised closed
grammar, exact-byte hashing including LF, publication order, exclusive
publication refusal, and partial write and flush failures. The Project tests
ran the real CLI and confirmed legacy/sealed artifact parity, nonce-independent
receipt bytes, silence on publication conflict, and unchanged derivation
diagnostics. The complete preimages and restoration recipes are in
`docs/evidence/codec-stdout-publication-seal-20261017/`.
