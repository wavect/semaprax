# Comment comparison fixture correction

Source-only correction atop `d6bf8151942190a956eb57350fa8b8892a8c68e5`.
The supplied qualification log fails before comparing any formatter output for
the second corpus case: module-only input correctly refuses with SPX-P101.

The complete original Unicode/CRLF/nested/trailing-comment source and both exact
module-only strings remain. Normal admitted source still takes the original
parser path. The two explicitly marked formatter-only edge cases now assert
their original parser refusal, append one temporary function for AST construction,
assert it is the sole function, and remove it before either formatter runs.
Comment rows are independently lexed from the exact original bytes and asserted
equal to those from anchored parsing. No production grammar admission changes.
The same helper constructs the empty AST when checking canonical idempotence.

All original exact byte-length, match/mismatch, and idempotence assertions remain;
the overflow/sticky-mismatch test is unchanged. The independent complete String
formatter, comparison sink, lexer, placement implementation, and existing comment
corpus are unchanged. This repairs test construction rather than claiming an
observed production formatter defect.

`source-inverse.patch` reconstructs the complete original changed test file from
this correction. Only this small local inverse and metadata are recorded; no raw
repository snapshot or global composition was made. Hashes and exact pending
controls are in `source-proof.json`.

No Cargo/build/test/Clippy/native execution was performed. `git diff --check`
reported no whitespace errors. Compilation and all listed executable controls
remain pending with the parent; this artifact is not passing-test evidence.
