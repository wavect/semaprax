# Compiler stdout publication seal — NEXT source candidate

Base: `57f8758749efe32ebadf99149223dd8a8abba0f2`. Private SSD lane; ROOT/current frozen qualification was not edited. Scoped to compiler-side [OPT869](https://github.com/wavect/semaprax/issues/869), linked to OPT606. This batch closes neither issue.

`--receipt-stdout-seal <nonce>` is optional and valid only for the retained-receipt route. The nonce is exactly 1..64 ASCII letters/digits/underscore/hyphen. The original receiver still performs all ordinary parsing, Project authentication and two actual derivations. Without the new flag it calls the unchanged publisher and emits no new stdout bytes. With the flag it hashes the exact already-encoded JSON+LF buffer, prepares a bounded record, exclusively publishes the original buffer, then writes and flushes that record. No second artifact, path rehash, process, environment access, or authority is added. Partial-write/flush failure reports that publication succeeded and leaves the artifact intact; consumers need successful status as well as valid output.

## Preserved source and checks

`before/` holds complete original bytes for every one of the seven changed files. `source-cuts.json` authenticates full before/after files and explicit finite inverse edits. The old complete receiver body, ordinary CLI parser/publisher, existing whole integration assertions, old raw-join tokens and original contract text restore exactly. Seven unchanged whole sources are pinned separately, including compiler Project capture/repeat code, old grammar/unit owners, and host accounting/receipt validation. No host authentication flags change.

The source verifier reports PASS_SOURCE_ONLY: seven complete preimages, four new files, seven unchanged complete files, 74 old assertion calls in the changed Rust inputs restored, three raw source joins and 34 drift controls. Controls exercise same-length whole-source changes, appended drift, and missing/duplicate/foreign additive raw-join tokens. Explicit new child registrations are checked. This is source authentication, not Rust compilation or executable behavior evidence.

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 /opt/homebrew/bin/python3 docs/evidence/codec-stdout-publication-seal-20261017/verify-source.py
```

Rustfmt edition2021 with skip_children parsed/formatted only the two changed receiver/helper files and two new test files. `git diff --check` passed. No Cargo, Rust compiler, provider, RPC execution, or runtime test was run. All eight new Rust owners and the two preserved owning regressions in `pending-owners.json` remain PENDING. Scoped compilation/Clippy/runtime qualification is required before any support claim.

## Integration and finite custody impact

Changed production roots: `src/cli/json_codec.rs` (new opt-in dispatch), `src/cli/json_codec/retained_receipt.rs` (explicit helper registration and publication callback). New production child: `src/cli/json_codec/retained_receipt/stdout_seal.rs`; it is ordinary CLI code, not a HIR/expected-projection purity boundary. New test-only children: the helper's explicitly path-registered `stdout_seal/tests.rs`, and `tests/project/standard_library/application_json/retained_receipt/stdout_seal.rs`, explicitly registered by appending to the complete old retained-receipt integration module. No new top-level integration binary.

All three complete raw source joins receive exactly the same three new paths; native inventory additions stay inline and the file remains1500 lines. Every old token and raw input byte remains present. Existing finite predecessor scripts/raw-source role manifests must receive an exact additive successor binding when this NEXT batch is joined. No historical endpoint or original script was regenerated here. Preserve this packet's before/after endpoints as a reviewed layer; do not overwrite prior archives or claim parent-final metadata/runtime qualification from this private check.

The host side still needs independently qualified execution-origin, immutable executable provenance and authenticated effective sandbox/environment continuity. Existing agent-writable mailbox messages cannot authenticate the seal. See `docs/JSON-CODEC-PUBLICATION-SEAL-V1.md` and the external source-bound discovery `/Volumes/Semaprax/provenance/606-runner-owned-execution-origin-discovery-20261017.md` (contains the reviewed follow-up issue draft; parent has filed #869).
