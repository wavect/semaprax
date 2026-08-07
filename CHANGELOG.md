# Changelog

All notable changes to SEMAPRAX are documented here.

## 0.2.0 — 2026-08-07

- Added resource declarations and explicit `own`, `borrow`, and `shared` boundaries.
- Added stable resource identities and atomic resource/type-boundary renames.
- Added straight-line move analysis with use-after-move and illegal-transfer diagnostics.
- Exposed resource nodes and parameter ownership in the semantic graph.
- Added a direct WebAssembly core backend and generated browser package.
- Preserved checked `i64` arithmetic through audited WebAssembly host imports.
- Added the authoritative full-goal completion matrix.

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
