# Mechanical correction after actual23e8 compiler failure

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only patch based exactly on23e8f1d0a67cb2657bb4cdc963a2a2828c523c7c.
The parent owns the complete failed build receipt and the semantic UpdateField
match correction. This packet corrects the other four actual errors: one explicit
receiving-test module path and three SHA256 formatting calls through the existing
`crate::digest_hex::LowerHex` byte wrapper. Hash assertions and expected values are
unchanged. It also gives `new_project::application` an explicit path that resolves
to the same existing file under ordinary full-CLI compilation and custom-path
integration inclusion; all six test functions remain.

All nine actual warnings are addressed without suppressions: unused imports or
re-exports are removed while their function definitions and independent old
receivers remain intact. The unused expected Diagnostic local is renamed
`_expected`, preserving its initializer and destruction. No test assertion,
semantic function body, checked profile, or admission limit is deleted.

Every changed file has its complete immediate source archived under `original/`,
SHA/line/byte records in `source-proof.json`, a complete unique current-to-original
inverse in `source-inverses.json`, and the additive patch inverse. No earlier84
packet metadata was changed and no global composition ran. Parent must add these
new source inverses to its final composition after merging this packet and its
separate semantic fix. No source custody `a.rs` growth or edit occurs.

`pending-selectors.json` lists the exact direct owners; the parent retains every
actual failed check and complete existing source/custody gates from its current
qualification manifest. The next compile must include the main library and the
separate full CLI/toolchain integration role. Static registration must use the
unchanged original module parser and these explicit source paths; a stem-directory
fallback is not evidence that Rust registered a child. The old external static
adapter/receipts are historical unexecuted artifacts, not runtime qualification.

No Cargo, build, tests, Clippy, C, Node or Claude was run in this lane.
`git diff --check` passes. Compilation, warnings and runtime gates remain pending.
