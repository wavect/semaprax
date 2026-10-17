# Named byte owners in primitive String-vector fixtures

Status: source-custody checks passed; actual compiler and backend qualification pending.

Audience: compiler maintainers and reviewers.

The primitive String-vector response fixtures attempted `array_as_slice` on
byte-array literals. Current CI reports the existing SPX-T266 refusal: that view
requires a named owner. Four template sites now bind their byte arrays before
borrowing. The text expression helper uses a local block so each created String
is detached before its byte-array owner ends. Main expected-output buffers and
the maximum-output tail use ordinary named local bindings.

The complete predecessor test module is retained in
`string_vectors.before.rs.txt`. An executable owning source check restores the
full module with five exact site substitutions and authenticates its full SHA256.
Every prior case, value, independent serializer, diagnostic assertion, output
limit, backend and forged-plan control remains byte exact after that restoration.
No compiler admission, capacity, fixture value or expected outcome is changed.

A new compiler regression checks the helper's named-owner expressions, canonical
round-trip and HIR resolution, and retains SPX-T266 for a temporary byte array.
The focused source-custody check executes its full actual body on Rust 1.98 under the unchanged 200 MiB aggregate child limit. Full compiler execution,
the original cross-backend cases and whole current-head CI remain required.

The 8c08 current-head run then reached an existing reserved local binding named
`result` in the item/byte plus-one fixture. It now uses `outcome` for the same
encoder result and match. The complete-source restoration also reverses that
rename; all original assertions remain. A stable diagnostic control retains
SPX-S109 and SPX-T201 for a reserved local binding and reference. Its real
compiler execution awaits the successor CI run.
