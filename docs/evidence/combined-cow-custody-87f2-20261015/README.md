# Final combined COW source custody

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Immutable source input: 87f267a62 (full SHA in combined-source-proof.json). Metadata only; no Rust, Cargo, Clippy or runtime execution.

The explicit prior active 24-packet/292-row list is preserved, with Session19, its compile-correction2 and Legacy18 added (27 packets/331 rows total). The nominal v14 79 rows are included in that active set. Sixty-six additive complete-current-to-authenticated-prior inverses compose the later COW and Session changes. Every old hook, original hash/archive and selected historical proof byte stays fixed. Independently bound new files retain their original explicit binding semantics.

Run `python3 docs/evidence/combined-cow-custody-87f2-20261015/compose.py --verify-only` from the checkout. The zero-repair second pass verifies all current bindings and old-source projections plus the complete existing source36. The original e7 helper and proof remain unchanged; this is a new proof snapshot. No source or archive is rewritten.
