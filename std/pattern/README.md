# Bounded ASCII byte patterns

Partial alloc-tier standard-library package. Its optional `api` selection in
`std/packages.json` exposes the supported catalogue and direct-import
conformance surface; it is metadata, not a source privacy boundary. The
package adds no public host ABI or completed-module claim.

A library consumer can use the existing `owned-data-api.v1` Project profile and
`[dependencies] std.pattern = "^0.1.0"`. Import `std.pattern.matcher` as a
type and the selected functions by stable identity from `std.pattern`. The
compiler supplies the immutable bundled implementation; the application does
not copy its source. [examples.spx](src/examples.spx) compiles once and renews
one Matcher across independent named inputs.

Start from that example's stable-ID imports. For instance, the type, factory,
and `compile` imports are:

```spx
use type @id("std.pattern.matcher") from std.pattern as Matcher;
use function @id("std.pattern.make") from std.pattern as make;
use function @id("std.pattern.compile") from std.pattern as compile;
```

The names after `as` are local aliases. Calls use those aliases; dependency
selection belongs in the manifest's `[dependencies]` table, not in the source
file. The standalone library consumer uses `[package]` profile
`owned-data-api.v1`; the private [LogLens package adapter](../../experiments/ascii-pattern-source/loglens-reference/matcher-package-adapted/semaprax.toml)
shows the explicit `source-command.resource-output.v1` command profile and
its capabilities. Each profile keeps its existing semantic admission.

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

`full_match` already anchors the complete input. Raw `^` and `$` outside a
class are invalid; use escaped punctuation when those bytes are literals.
Use byte classes such as `[0-9]` and `[ \x09]` for digit and space/tab matching.
Parenthesized captures are flat and numbered from zero in opening order;
repetition applies to one byte atom, not to a captured group.

The pattern parser receives bytes after the SEMAPRAX string parser has
decoded the source literal. Double each pattern backslash in `.spx` source:

| Pattern bytes | SEMAPRAX string literal | Meaning |
| --- | --- | --- |
| `\x41` | `"\\x41"` | One byte `A`. |
| `[ \x09]+` | `"[ \\x09]+"` | One or more spaces or tabs. |

The exact class, escape, and greedy rules are in the owning
[pattern specification](../../experiments/ascii-pattern-source/DRAFT.md#pattern-meaning).

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

Malformed-pattern detail selects the first offending byte from left to right; a missing byte reports EOF at the pattern length. A dangling backslash therefore has pattern detail 1, while an unsupported first byte can have detail 0. Both use detail domain 1.

Packet observers other than `result_valid` require a valid packet. Numeric capture endpoints grant no ownership or input authority. The consumer remains responsible for applying them to the input of that match. Borrowing a view of the renewed Matcher storage still fails ordinary ownership/loan replay; only independent named views qualify.

For a reusable parser, keep the owner returned by `compile`, require a valid
packet with status 0, then pass that whole owner to each `full_match` and keep
its returned owner. Name the pattern and input views independently, as the
example does with `string_as_str`, `str_as_bytes`, and `array_as_slice`.
Decode capture endpoints only for status 1 with an index below
`capture_count`. Status 2 means semantic no-match; statuses 3 and 4 require
separate invalid/resource handling. The named
[EOF and escaped-byte regressions](src/tests.spx) also show exact detail and
work witnesses without confusing zero offset with success.

Capture endpoints are relative to the exact input view passed to that match.
For a line cut from a larger file, apply them to that line; conversion to an
enclosing-file offset must add the line's start. Applying them through
`string_slice` retains its ordinary bounds and UTF-8 boundary checks.

The bounded engine may refuse ambiguous searches before deciding a semantic match. Logical work and ordinary interpreter AST fuel are separate limits. No limit is raised here: final qualification must preserve the complete LogLens 49 obligations, independent header/key-value meanings, long valid and nonmatching records, exact work witnesses, hostile carrier controls, and interpreter/native C11/Core-Wasm settlement on these exact source bytes.

`std.pattern.internal.*` declarations remain explicit, checked implementation source. API inventory selection controls supported interface documentation; it is not source privacy and conveys no capability.

The three-backend conformance gate retains the original owned-String and
concurrent-Matcher fixtures. Its full mixed-resource Wasm arena requires the
authenticated cleanup inventory's four Matcher payloads plus four Strings for
tests, and one String plus the renewed Matcher for examples. Both roles assert
the actual live-entry peak, refuse the adjacent lower bound, and settle to an
empty arena on four reentries. An additional borrowed-input conformance project
uses the real library under the strict one-entry bound, retaining all diagnostic
byte cases and result observers; its zero-entry negative reaches the actual
allocation refusal. The original shipped source is unchanged.
