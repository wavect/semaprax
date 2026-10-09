# Bounded ASCII byte patterns

Partial alloc-tier standard-library package. Its optional `api` selection in
`std/packages.json` exposes the supported catalogue and direct-import
conformance surface; it is metadata, not a source privacy boundary. The
package adds no public host ABI or completed-module claim.

A consumer uses the existing `owned-data-api.v1` Project profile and
`[dependencies] std.pattern = "^0.1.0"`. Import `std.pattern.matcher` as a
type and the selected functions by stable identity from `std.pattern`. The
compiler supplies the immutable bundled implementation; the application does
not copy its source. [examples.spx](src/examples.spx) compiles once and renews
one Matcher across independent named inputs.

The selected API consists of these stable IDs, in canonical sorted order:
`std.pattern.capture-count`,
`std.pattern.capture-end`, `std.pattern.capture-start`,
`std.pattern.compile`, `std.pattern.detail`, `std.pattern.detail-domain`,
`std.pattern.full-match`, `std.pattern.make`, `std.pattern.matcher`,
`std.pattern.reason`,
`std.pattern.result-valid`, `std.pattern.status`, and
`std.pattern.work-used`. Internal helpers
are deliberately absent from that catalogue selection; they remain present
and checked in the ordinary source module.

`make()` allocates exactly one 3,072-byte carrier. Its fresh bytes have no result packet: call `compile` before the packet observers. Compile takes one whole owner, a named borrowed pattern view and a logical-work limit from 33 through 262,144. `full_match` takes that whole owner, an independent named borrowed input and a limit from 32 through 262,144. Both return the owner. Compile invalidates the prior ready byte before parsing; matching preserves the compiled table after every packet. No input view enters the carrier, and neither operation allocates or grows it.

The unchanged grammar admits ASCII literals, byte escapes, dot, classes, bounded/greedy repetitions and flat capture groups. It limits patterns to 1,024 bytes, inputs to 65,536 bytes, atoms to 128, distinct classes to 16 and captures to 16. Captures are byte offsets, including possible UTF-8 byte boundaries. General alternation, Unicode character semantics and unrestricted regular expressions are outside this API.

| Observer | Meaning |
| --- | --- |
| `result_valid` | Checks the complete logical packet, including every reported capture span; fresh or malformed carriers return false. |
| `status` | 0 compiled/ready, 1 matched, 2 no-match, 3 invalid, 4 resource refusal. |
| `reason` | 0 none; invalid 1 pattern syntax or 2 compiled table; resource 1 pattern length, 2 input length, 3 atoms, 4 classes, 5 captures, 7 work. Reason 6 is reserved and rejected. |
| `detail` | The packet detail interpreted through its domain; zero may be a valid first-byte offset. |
| `detail_domain` | 0 none, 1 pattern-byte offset, 2 compiled-table-byte offset, 3 pattern-byte length, 4 input-byte length. |
| `work_used` | Logical compile/match work actually charged. Observers never debit this meter. |
| `capture_count` | Number of matched capture spans; zero for every other status. |
| `capture_start` / `capture_end` | Numeric endpoints; require matched status and an index below capture count. |

Packet observers other than `result_valid` require a valid packet. Numeric capture endpoints grant no ownership or input authority. The consumer remains responsible for applying them to the input of that match. Borrowing a view of the renewed Matcher storage still fails ordinary ownership/loan replay; only independent named views qualify.

The bounded engine may refuse ambiguous searches before deciding a semantic match. Logical work and ordinary interpreter AST fuel are separate limits. No limit is raised here: final qualification must preserve the complete LogLens 49 obligations, independent header/key-value meanings, long valid and nonmatching records, exact work witnesses, hostile carrier controls, and interpreter/native C11/Core-Wasm settlement on these exact source bytes.

`std.pattern.internal.*` declarations remain explicit, checked implementation source. API inventory selection controls supported interface documentation; it is not source privacy and conveys no capability. Public nominal host descriptors and opaque host handles are separately scoped and remain unselected.
