# SG-20/SG-21 actual format-61 fixture

These are unmodified local review artifacts from `semaprax-main-gaps-20261007`.
`review_index.json` was emitted by pinned `nightly-2026-10-02` rustdoc on Linux;
`capture-results.json` retains exact capture commands and stable rustc positive
control results. Their historical source-root path is retained for span replay.
The fixture is compiler metadata and local evidence, with no execution authority.

The converter boundary suite re-extracts it, selects the public module alias,
and checks the enum payload closure. Unknown variant shapes and bounds retain
separate synthetic boundary tests. The Rust source can be checked independently
using stable rustc `--test`; no nightly installation is required for replay.

During focused local verification, the same retained source also passed its one stable test on
Darwin arm64 (`rustc 1.98.0 (88d9e12ae 2026-08-18)`). The installed pinned
`rustdoc 1.101.0-nightly (c36f14571 2026-10-01)` generated a fresh format-61
JSON document (118,932 bytes), which the fixed converter extracted successfully:
`review_index::public_api::increment` is public/selectable, and `make_event`
reaches both `Event` and `Payload`. Those fresh temporary artifacts were removed;
the original Linux capture above remains the committed replay fixture.
