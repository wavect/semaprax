# Exact entry stages for legacy CI source inverses

The unchanged ordinary-setup audit failed at main dbf6fed71b because its
CI input inventory received the later replay and factored source bodies.
Both required inputs already exist as exact named whole views in the current
source table. The adapter admits only those two literal source/digest pairs.
It authenticates the complete current compiled, physical, and registered
channels before selecting a predecessor. The original successor checks,
indexed inverses, archive comparisons, and mutation/forgery controls still run.
The actual current source supplied by the caller is checked before projection.

Every original test body in ci_inputs.rs remains byte-identical. Local checks
use the actual source helper harness and warnings-as-errors Clippy. This is
source-only qualification; it is not full current-head CI success and changes
no compiler, allocator, or runtime authority.
