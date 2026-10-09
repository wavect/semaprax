# ShiftSim source 398 native v3 qualification

Unpaid qualification session 71437 exited 0. The archived source 398 compiler checked the retained v27 candidate, built a fresh native executable, and passed all 15 frozen acceptance cases: 11 valid and four invalid. The two inputs over 65,536 bytes, compact maximum-cardinality control, and invalid capacity boundaries remain required. This is current compiler/reference qualification; no paid model request, token-cost result, or new comparative campaign is recorded.

The exact evidence and companion text artifacts are retained here without rewriting their bytes. [`qualification-evidence.json`](qualification-evidence.json) uses absolute paths to the original external artifacts, including the qualified native executable. That binary and candidate source tree remain external; this directory is a publication snapshot rather than a self-contained executable bundle. Its hash-bound input inventory and manifest identify the actual retained candidate; prior paid campaign evidence is not reattributed to this qualification.

| Binding | SHA-256 or value |
| --- | --- |
| Compiler source | `398b051e6e7a06ac49ecf77d9292831401430de0` |
| Archived compiler | `594980a96bb3d7f74a168dfa6bc71d6355344f2963489f1e3ae854d2f3a01237` |
| Compiler identity | `7819129f86f391dd049257bd5a6ed09b17f191575d8e5b41e34a6e8ff6f6acdb` |
| SPEC | `5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00` |
| Acceptance corpus | `3c285999cfcf6a905e885d636ac55ba0ccb0f5999ef5b20ac3a8c17a2e023587` |
| Evidence envelope | `6b41f57873ea94954c121539c99f28ab96663c1fa00ecd5316f5b8bb749f33d8` |
| Acceptance report | `6d2edb5c32701c57d8f4b8c2c065c2e0bbd2951c1db88598c2dcda62c88dc1af` |
| Source inventory file | `b8a594149b1b12ad12e27519dfd983fa0fd1b14160c482ed2594dafbe5c4a738` |
| Closed authored inventory | `17d0d9da00dea8378e904436837ee5b3930916d71f58482dc2aa5c0a3ab0eb8f` |
| Candidate manifest | `d3687236ce9319a609262111f5ceb60e244e0227143d6a3811d75745469e94fd` |
| Qualified native binary | `dd44f27ad29bfcb60703740a6c7340d99fa43c32771e13ce0a3abe8dbd03e6f7` |
| Qualification build receipt | `49ee6e7b7691760783ee75353a0850001142a7d28f071368e04f03b1a1d7458f` |
| Final terminal receipt | `be6896823cc52a95467e9d3c45f82745f7513502e4df539626badfa57ecce7b4` |

The route is Project v27 / `language-command-io.stream-data.v1`, with `argv-utf8+stdin-stream.v1`, `fn() -> i64`, and process status 0–255. [`qualification-build-receipt.json`](qualification-build-receipt.json) binds the compiler, closed authored source inventory, exact manifest, fresh native binary, and acceptance report to the same subject.

The command ran from 02:24:51 to 02:25:09 UTC on 2026-10-09. Those 18 seconds overlap independent heavy Cargo/test work and are not an isolated timing or compiler-speed comparison. Eight retained host observations recorded a minimum free disk of 6,757,249,024 bytes, at most two heavy roots, no unexpected external roots, and 73% system-wide free memory at each sample. The 5 GiB resource floor was satisfied. No Cargo was invoked by this qualification, and paid admission remains separate with zero heavy jobs required.

The [original monitor receipt](terminal-receipt.json), SHA-256 `e0a8f72ee0f62edba94d76c8b1824ce8291eaf86ebeeae09e0dfe334848dbd7c`, preserves its post-run validation false negative: it compared the structured `native_project_route` object to a string and omitted the preflight row from its observation counter. The [final receipt](final-terminal-receipt.json) corrects those read-only checks, validates the existing hash chain and all eight observations, and leaves the original receipt intact. Qualification was not rerun.

Original output: `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/shiftsim-qualification-source398b051e6-20261009/qualification-v3`. Execution logs and host observations: `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/shiftsim-unpaid-source398-qualification-prep-20261009`. The exact command is retained in `terminal-receipt.json`; it invoked `campaign.py qualify-v3` with the explicit source 398 commit, retained candidate, archived compiler, fresh output, and 1,800-second per-phase timeout. This evidence permits later qualification review, but claims neither paid wrapper admission nor a new token or language advantage. Earlier qualification pins and paid reports remain historical and unchanged.
