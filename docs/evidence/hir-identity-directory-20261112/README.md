# Paid HIR identity directory qualification (#805)

The HIR validator uses the ordered identity directory introduced in
`f8465255f`. It pays for each new AVL node before construction and stages
shared or weak suffixes before mutating a unique prefix. The saved validator
still runs with the original B-tree receiver and its per-insertion reservation.

On `7268e5fbe`, all ten focused identity tests and five complete validator
comparisons passed. The checks covered duplicate priority, expression and value
ordering, closure merge prefixes, exact and one-byte-short reservations, sticky
refusal without another allocation, snapshots, and Drop. The validator
comparisons checked the full diagnostic and state on success and late failure.
Three additional generic and function-scope controls passed.

The existing constructor tests reported the following observations on macOS
aarch64 with Rust 1.97.1. The measured constructor, key, budget, and allocator
observer source files in the debugger build at `114d7434f` were byte-identical
to `7268e5fbe`.

| Receiver | Keys | Allocator requests | Requested bytes | Cumulative bytes acquired |
| --- | ---: | ---: | ---: | ---: |
| Saved B-tree, expression identities | 64 | 10 | 1,136 | 13,312 |
| Paid AVL, expression identities | 128 | 128 | 7,168 | 12,288 |
| Saved B-tree, value identities | 64 | 10 | 2,896 | 24,576 |
| Paid AVL, value identities | 128 | 128 | 9,216 | 16,384 |

Every row released all requested bytes, had zero live bytes at the end, and
made no reallocations. The AVL rows also checked ordered cursors and two
allocation-free snapshots. These are the existing fixtures at their recorded
sizes. Shared suffix copies and different insertion sequences have their own
allocation profiles.

The unchanged Catalog and ShiftSim fixtures and all 13 Catalog Normalizer
checks passed under the ordinary builder ceiling on `7268e5fbe`. LogLens also
passed all 33 historical and 16 boundary checks through the native executable.
Production Clippy passed. The workspace, calculator, and projection outputs
in the paired capture were byte-identical. The existing workspace and browser
assertions still failed against their older pins on both revisions.
[Paired qualification](../record-presence-receivers-20261112/README.md)
