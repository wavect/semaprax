# Public graph and analysis accounting expectations

The previous exact expectations are preserved in the complete 2026-10-26 wire records. This update keeps every semantic field unchanged. It accounts for earlier builder savings, the already-existing 128 MiB workspace builder limit (the old records contain 64 MiB), serialized decimal-length changes, and the independently authenticated digest cascade. This change raises no limit and removes no assertion.

`verify.py` checks all 18 complete old/current wire pairs, their raw SHA-256 values, each exact changed field, all self-digests and nested graph bindings. It reconstructs every complete current byte stream from its complete predecessor using only those verified changes.

macOS wires were captured from the unchanged original tests at `e66858fe9b70ecf6432bdfaea7271012356d7a78`. Portable wires are actual historical Linux CI outputs from job 126103071589 at `e04bd5d131b2f625b34806456e565718ebf8c11c`, independently identical to the later 879 Linux capture. Current Linux CI remains authoritative for the new head. No future Linux measurement is claimed.

Run `python3 docs/evidence/public-wire-accounting-20261104/verify.py`. The two original public API/CLI tests retain whole-byte comparisons, getters, semantic assertions, shared locking, no-write inventory checks and hostile controls.
