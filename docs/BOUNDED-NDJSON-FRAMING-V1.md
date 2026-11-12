# Bounded NDJSON framing v1

Audience: compiler contributors and reviewers.

Status: focused package and Catalog Normalizer qualification passed on macOS arm64. Matched agent trials remain pending for the bounded LF-only composition tracked by [OPT #760](https://github.com/wavect/semaprax/issues/760).

## API and authority

`std.data.ndjson` is an ordinary checked source package depending on `std.io.lines` and `std.data.json.utf8`. It has no host imports, Web exports, borrowed-return exception, new byte carrier, or allocation intrinsic. Public declarations and fields have persistent IDs in `std/data-ndjson/src/ndjson.spx`.

`record_count`, `line_start`, and `line_end` observe an immutable borrowed Slice. `frame_at` returns a Copy `Frame { ordinal, start, end, next }`; `frame_matches` reconstructs those numeric cuts from the current body. A Frame is not a span certificate: another body with identical cuts can satisfy it. There is no owner token or generation in the record. These random-access helpers rescan the prefix/body; they are not a linear batch cursor. The adopted coordinator retains its own single count and monotonic cuts.

`basic_fault(body, frame)` first requires numeric agreement, then forms an ordinary `byte_range` in the receiving function. `basic_line_fault(line, ordinal)` receives an actual borrowed view, permitting a coordinator that already knows its current cuts to avoid repeated prefix reconstruction. Both routes retain independent source/HIR byte-range origin, extent, operation, identity and loan validation. Numeric facts grant no authority to move/drop an owner, forge a root, return a Slice, or retain it after settlement. The existing rules in [Portable indexed byte data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md) remain mandatory.

## Bounds and faults

The selected framing policy is a body of at most 65536 bytes, at most 256 records, and each line at most 512 bytes. LF separates records; CR remains content. Empty input and a lone LF have zero records. A final LF creates no additional record; interior empty lines fail. Frame intervals exclude LF, and `next` includes it when present. No UTF-8 or JSON normalization occurs during framing.

`Fault { category: i64, record_index: i64, byte_offset: usize }` is a Copy observation, not a language failure status. Category 0 means no framing fault, 1 a size/count limit, 2 malformed UTF-8, and 3 an empty record. Global faults use index -1 for the body and 256 for excess record count. Record indices are zero based and byte offsets are relative to the line. Whole-body size precedes record count; per-line size precedes UTF-8, which precedes empty-record rejection. UTF-8 offsets retain the existing earliest failing sequence-start convention. JSON grammar, field rules, decoded duplicate detection, checked accumulation and final publication remain in the consuming coordinator.

`line_end` requires a start within the current Slice; `frame_at` requires bounded body and a present ordinal; `basic_line_fault` requires ordinal below 256. The adopted legacy Catalog Normalizer wrappers retain their previously total behavior: out-of-range starts return the original start, and ordinals outside the new profile use the unchanged original fault computation. No strengthened legacy precondition is introduced.

## Composition and qualification

`examples/catalog-normalizer-project/src/batch.spx` forms the actual receiving range and maps structured faults back to its unchanged packed application error representation. Strict JSON/field ordering, decoded ID duplicate priority, checked totals and all-or-nothing publication remain unchanged. The frozen [Catalog Normalizer oracle v1](CATALOG-NORMALIZER-ORACLE-V1.md) still owns all application behavior and its 160M fuel ceiling. Catalog23 remains a separate regression.

The `standard_library::ndjson` Project tests passed on macOS arm64. The interpreter, C11 O0/O2 and Core Wasm ran the package examples and conformance checks, including exact and one-over framing limits. The equal-cut case reuses a numeric frame with a different body and reports the UTF-8 error from the current bytes. The HIR verifier refused foreign roots and forged types, operations and producers. Overlapping or out-of-range cuts selected the exact requires-false status across repeated calls, with poisoned result storage and zero live owners. The native probe observed zero heap allocations for these Array-backed refusals.

Both cold and warm checked-cache builds preserved the complete authenticated Project graph byte for byte. The warm build reused all six checked modules, including bundled dependencies, and passed HIR validation. The catalog producer ran and the committed catalogs matched its output.

All 13 Catalog Normalizer tests passed. The differential ran the saved original batch source and the structured receiver. Both matched all 36 published oracle cases in the interpreter and ran the 14 declared cases in the interpreter, C11 O0/O2 and Core Wasm. Both produced the exact maximum valid responses within 160 million interpreter steps. The separate Catalog23 restock regression passed all 23 original cases.

These runs establish functional and backend qualification for the exercised profiles. Fresh matched agent runs are still required to measure authored tokens, turns and accepted-task cost.
