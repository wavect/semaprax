# Historical native archive capture

Baseline: `4dd73795021564dcc6d90ac6505b2814c1818cd1`.
Target: `aarch64-apple-darwin`.
Tools: Rust 1.98.0 (88d9e12ae 2026-08-18), Apple Clang 21, `/usr/bin/libtool`.

The expected digest rows were captured by executing the historical package's
`build_and_publish`, with this directory's `owned_v1_archive_provider.c` and the
historical `descriptor_bytes("owned-bytes")` test helper, in standalone evidence
mode. No current package renderer generated the expected rows.

The historical crate was extracted with `git show`/`git ls-tree`; every
production input was compared byte-for-byte against its immutable Git blob.
Only a test registration and capture test were appended. Direct rustc `--test`
linked the same warm serde_json, sha2 and platform rlibs used by the current
owning selector. Both platform crates' source directories and Cargo manifests
were verified unchanged between baseline and current source. There was no
historical Cargo build or second target directory.

The capture used a fresh canonical absolute output path and held configured
Clang/libtool through the ordinary publication implementation. The initial
noncanonical `/tmp` spelling was refused before effects; canonicalization was
required. The capture read each published file and recorded its name, byte
length, and raw SHA-256 in sorted filename order. That order is solely fixture
inventory order, unrelated to language cleanup order.

The current owning test rebuilds and publishes the same input, compares every
row including the actual native archive, and links/runs a Rust consumer against
it. A second provider changes payload byte 0xff to 0xfe: it must alter the
archive and fail the consumer's payload assertion after successful compilation.
The real provider proves separate allocation/copy/free/close behavior; the
previous fixed-sentinel archive digest test is a distinct text-rendering gate.

Scope: arm64 macOS with these tools and current locked dependencies. Historical
whole-toolchain reproducibility, cross-platform native archive identity and
hosted execution are not claimed. The test module is target-selected accordingly.
