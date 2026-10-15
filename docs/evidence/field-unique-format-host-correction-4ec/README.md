# Private field uniqueness formatter host correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual current350 failure257 linked the admitted 4096-row String corpus against the existing strict private owned-leaf Vec host, which lacked the actual compiler-selected spx_format_step_v1 import. The checked-literal-format v1 ABI and aggregate emitter require (i32 operation,i64 left,i64 right)->i64 owned result. There is no fuel debit in this import. This test-only host now implements the same production browser formatter operations: copy0, UTF-8 checked concat1, signed decimal2 and unsigned decimal3, with the unchanged 65536 result bound, operand validation, token exhaustion and RangeError refusal returning zero. Results enter the existing ownBytes allocator and copy/drop inventory; no bypass handle or dummy result is introduced. The standalone browser table's 64-entry policy belongs to its different adapter; the strict Vec host retains all existing collection/leaf/copy-codec capacities and accounting. No production adapter or cap changes.

A missing-import link control proves this exact private boundary is required. The original complete Rust corpus remains byte-for-byte unchanged: 4096 and late duplicates, two owned leaves, all interpreter/native/Wasm engines, complete rejected original Bytes loop, clone/sort oracle, declaration identity, order, capacity and scratch refusal controls remain authoritative. The actual full receiver must rerun in ROOT; no Cargo/build/runtime ran here. This change makes no application or public browser support claim.

The complete prior host and failed log are retained with an exact whole-source inverse. Existing sources remain fully in their original files; nothing is relocated outside source scans.
