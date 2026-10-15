# Receiving test digest formatting API correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Base: `93fdc103c89fda0230538e568b3f8f661c55f399`.

The actual focused build reported seven `LowerHex` trait failures in the two
new receiving-test modules. Each call now wraps the unchanged Sha256 digest in
the repository's existing allocation-free `crate::digest_hex::LowerHex` adapter.
No producer, expected hash, source inverse, diagnostic, quota or production
receiver changed. Three calls are in `regions_entry/tests.rs`, four in
`remaining/tests.rs`.

Full original test modules are archived with pinned SHA-256 and exact formatted
source inverses. Python verified both inverse reconstructions and equality of
all expected 64-digit hash literals. The original owning source-inverse files
and archived production sources are untouched. Rust formatting and diff checks
pass. No Cargo/build/runtime gate executed in this private lane; root must
rebuild the existing frozen focused selection.
