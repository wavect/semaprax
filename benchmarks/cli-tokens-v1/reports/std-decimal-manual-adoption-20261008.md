# Manual std.int.decimal adoption, 2026-10-08

An external copy of accepted LogLens S01 now imports ordinary
`std.int.decimal` normalization, addition and division through native Project
v26 `source-command.v1`. It preserves the report logic, argv/file capability
boundary and byte-identical original launcher and candidate tests. No library
source was flattened into the application, and no host arithmetic was added.

Compiler source `3e15d4bcfc1a7732b2afc125360dacd898a046c3`, binary SHA-256
`08ffeea70a6b690b82e7bc7e8a1ca5d6f08e84cf195b75f2eb6f82cc2eebd9e6`,
built the ordinary bundled-dependency Project as a native executable. The
unchanged candidate tests passed all **148 CLI checks**, the archived historical
harness passed **33/33**, and the independent SPEC boundary corpus passed
**16/16**, including decimal values beyond machine integer ranges and the exact
64-KiB file boundary. The complete original archive was verified unchanged.

The final authored inventory, using the original pinned legacy Claude tokenizer,
is **7,020 tokens for S01 and 6,898 for the adoption copy: 122 fewer (1.74%)**.
This is a modest manual inventory change. It includes canonical formatting,
imports, a Project manifest and structural test module, build wrapper and README
changes. The original single-file source first refused Project admission with
`SPX-G170`; the copy was then formatted by the current compiler. The recorded
difference is therefore not an isolated causal estimate of the library API.

This is a manual adoption prototype, not a live-agent trial, provider-token,
turn-count or billing gain. It measures final candidate files, not cumulative
editing or the shared compiler/library implementation. The profile is native
only; interpreter, Web/npm and Wasm support remain explicitly refused. This
receipt does not substitute for owning Rust gates or the full repository gate.
OPT #661 remains open pending the fresh matched live assessment; #667 additionally
requires its three owning tests and clippy before closure.

[Bounded summary and provenance](std-decimal-manual-adoption-20261008.json)
records file hashes, the exact tokenizer fingerprint, compiler provenance and
all boundary outcomes. Raw receipts and copied runtime artifacts remain at
`/Users/kevin/.codex/benchmark-runs/loglens-decimal-adoption-20261008/`; the
historical campaign artifacts are untouched.
