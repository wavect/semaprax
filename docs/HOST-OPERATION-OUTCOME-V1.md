# Host Operation Outcome v1

Status: **frozen design, not implemented.** No parser, HIR, verifier, semantic
graph, native, or Wasm change lands with this document. This is a versioned
specification for a semantic/authority contract change that, per issue #228
and the coordinator's own routing for the SPX-AI-019..025 series, gates behind
an independent review checkpoint and must not be self-approved by a bounded
worker. It exists so that checkpoint has one fixed target to review, instead
of an evolving one.

Audience: reviewers deciding whether to admit this profile, the implementing
agent once admitted, and anyone hitting issue #228's gap in the meantime
(directly: issue #123/#124's catalog-normalizer follow-on).

## Why this exists

Issue #228 names an exact hole: every fallible host operation in this
repository — the closed command-I/O table (`src/command_io_ops.rs`), the
network family (`src/network_io_ops.rs`), and the filesystem family
(`src/filesystem_ops.rs`) — **aborts the enclosing invocation on any nonzero
provider status** rather than returning an inspectable value. Concretely, for
`file_write_atomic`:

- `src/filesystem_provider.rs:14-22` closes `FileFailure` over seven variants
  (`InvalidPath`, `NotFound`, `AlreadyExists`, `CapacityExceeded`,
  `IoFailure`, `AuthorityDenied`, `InvalidFileType`) with a `status_code`
  (`src/filesystem_provider.rs:36-46`) — the provider boundary already
  distinguishes failure kinds.
- Native lowering discards that distinction before it reaches checked source:
  `src/codegen/native_emit/expression/host_command.rs:410` emits `spx_status
  = spx_host_file_write_atomic_v2(...); if (spx_status != SPX_STATUS_SUCCESS)
  goto spx_epilogue;` — the same unconditional fail-stop jump every other
  host-command call and every arithmetic/contract violation in the compiler
  uses (`src/codegen/native_emit/expression.rs:424`, `:799`, `:1132`, and
  eleven more sites; `src/codegen/native_emit/mod.rs:1897-1995` owns the
  shared `spx_epilogue` label).
- A checked handler therefore has exactly one observable bit after any
  `file_write_atomic` failure: the whole invocation stopped. It cannot tell
  "nothing was attempted" from "the atomic rename step may have partially
  applied before the provider reported failure" — issue #228's own three-way
  vocabulary: *validated and published*, *validation failed, nothing
  published*, *published then I/O failed, outcome uncertain*.

`docs/DURABLE-JOBS-V1.md`'s ["The #228
boundary"](DURABLE-JOBS-V1.md#the-228-boundary-what-blocks-a-checked-semaprax-caller)
section already reaches this conclusion independently while specifying
`std.jobs`'s `UNCERTAIN` lifecycle state (state `8`): the *decision procedure*
over an already-known uncertain outcome is fully checked today, but nothing
in checked SEMAPRAX source can *produce* the `kind == 3` (uncertain) input
that procedure consumes — only a Rust-side runner can, exactly the way
`DatabaseFixture::connection_lost` supplies `std.db`'s equivalent external
signal. That section also names "the exact probe a follow-on tranche needs
once #228 lands"; this document is that follow-on's design, still gated.

`tests/useful_data/filesystem_v2_native.rs`'s
`filesystem_v2_write_atomic_failure_collapses_into_undifferentiated_abort`
test, added alongside this document, pins the current behavior concretely: a
`write_atomic` provider callback that fails at the point a real "outcome
unknown" failure would occur produces the identical `!semantic_success &&
status_code == 5` shape, and the identical "nothing after it runs" abort, as
an ordinary pre-publication validation failure
(`filesystem_v2_rejects_malformed_list_wire_before_publication`, same file).
Nothing distinguishes them. That test must change deliberately, not by
accident, the day this design is lowered.

## Decision 1: a value-typed outcome, not exceptions or `try`/`catch`

SEMAPRAX has no general exception mechanism and this document does not
propose one. The repository's existing idiom for a closed, checked outcome is
a returned discriminant a caller matches exhaustively — `std.jobs`'s ten
lifecycle codes, the idempotent-enqueue three-way outcome
(`DURABLE-JOBS-V1.md`, "Idempotent enqueue"), and the `provider_outcome`
adapter in `tests/project/standard_library/provider_outcomes.rs` all follow
this shape already, all composed entirely in checked SEMAPRAX with no new
host operation. The gap #228 names is narrower: **no *host operation itself*
can hand back a closed outcome that includes a genuinely uncertain case**,
because every host-command status check is wired to the same fail-stop
`goto spx_epilogue` (native) / trap (Wasm) path used for unrelated internal
failures (arithmetic overflow, contract violations). Fixing that means adding
value-returning host operations whose own domain-specific failure classes
never reach the fail-stop path — not adding exceptions to the language.

## Decision 2: additive, not a breaking change to `file_write_atomic`

`file_write_atomic` keeps its exact v2 signature, status domain, and
abort-on-failure behavior (`docs/FILESYSTEM-IO-V2.md`, frozen). Every program
that already depends on "any failure aborts" keeps that behavior. The
three-way outcome is a **new, separately named operation** admitted under a
new additive profile, exactly as issue #228's acceptance criteria frame it
("if a new host op: define it under its owning versioned specification").
This also keeps the first slice honestly scoped to one operation: nothing
here proposes touching `net_send`/`net_recv` or any other family member.
Extending this taxonomy to network operations (the issue's other named
example, and `NetworkFailure::TransferFailed` in
`src/network_provider.rs:66-67` is the closest existing analogue — "the peer
reset the connection or a transfer failed midway", exactly a candidate
uncertain case) is explicitly **out of scope for the first slice** and left
as a follow-on once this shape is proven on one operation.

## Scope of the first slice

One operation: a new `core.host.file-write-atomic-checked` host command,
authored as `file_write_atomic_checked`, under a new `FilesystemV3` profile
additive to `FilesystemV2` (`src/command_io_ops.rs`'s
`CommandOperationProfile` enum, `:44-64`). Same signature shape as
`file_write_atomic` — `(borrow Slice<u8> path, usize path_length, borrow
Slice<u8> data, usize data_length) -> usize` — but its returned `usize` is a
closed three-code outcome instead of a byte count, and (unlike
`file_write_atomic`) a nonzero provider status from this operation's own
domain **never reaches `spx_epilogue`**; it is mapped to one of the three
outcome codes below and returned as an ordinary value.

### Closed three-way outcome taxonomy (`HOSTOUT-001`)

| Code | Name | Meaning | Maps from |
| --- | --- | --- | --- |
| `0` | `PUBLISHED` | The atomic replace committed. | `write_atomic` success (today's `Ok(data_length)` path, `src/filesystem_provider.rs:305` in the reference provider). |
| `1` | `NOT_PUBLISHED` | Validation or precondition failed before any mutating attempt; nothing changed on disk. | `FileFailure::InvalidPath`, `AlreadyExists`, `CapacityExceeded`, `AuthorityDenied`, `InvalidFileType` — every existing variant the provider can raise **before** attempting the rename/write step. |
| `2` | `UNCERTAIN` | The provider began the commit step and cannot confirm whether it completed. | A **new** `FileFailure::PublishUncertain` variant (this document proposes adding it under `src/filesystem_provider.rs`'s existing closed enum), raised only from the point a provider has started its atomic replace, never before. |

`NotFound` does not apply to a write operation and is not part of this
table. `HOSTOUT-001` is the frozen shape any implementer must lower
unchanged; widening it (a fourth code, a different code assignment) is a
change to this document, not to the implementation.

The critical admission rule, restated from the repository's own invariant
("failure selection is sticky; cleanup cannot replace the selected status"):
**`UNCERTAIN` is reached only from an externally observed
outcome-unknown signal from the provider itself, exactly the discipline
`std.jobs`'s `UNCERTAIN` state and `std.db`'s `connection_lost` transition
already both hold** (`DURABLE-JOBS-V1.md`, "The #228 boundary"). A provider
may not collapse an ordinary `IoFailure` into `UNCERTAIN` merely because
uncertainty is a documented case — `IoFailure` continues to mean "failed and
known not to have applied" and stays outside this table (it aborts, same as
today, via the existing `file_write_atomic` op unless the specific provider
implementation can prove the commit step was never entered). Only a provider
that genuinely cannot distinguish "committed" from "not committed" — a
disk write whose fsync or rename syscall itself returned an ambiguous
error, for instance `EIO` after the rename syscall returned rather than
before it was issued — may report `PublishUncertain`. That distinction is a
provider-authoring discipline this document states explicitly and a future
hostile-input test must check is documented, not silently violated.

## Capability requirements (`HOSTOUT-002`)

No new ambient authority. `file_write_atomic_checked` requires exactly the
effect `file_write_atomic` already requires (`WRITE_EFFECT`,
`src/filesystem_ops.rs:83`) — declaring `uses { fs.write }` is necessary and
sufficient, identical to today. This document does not add a "may report
uncertain outcomes" capability distinct from ordinary write authority: the
uncertainty is a property of the *outcome*, not a distinct grant of power, and
adding a second effect for it would let a caller select "abort on I/O
ambiguity" versus "observe I/O ambiguity" per call site, which is exactly the
kind of caller-selectable authority the repository's capability model refuses
elsewhere (a caller cannot opt out of a diagnostic by omitting a capability).

## Diagnostics and admission decisions (`HOSTOUT-003`)

These are the exact diagnostics a lowering must produce; each is stated so a
reviewer or implementer can check a diagnostic string against this table
without re-deriving it.

1. **Undeclared effect.** Calling `file_write_atomic_checked` without `uses {
   fs.write }` produces the existing diagnostic verbatim:
   `"host-command operation requires undeclared effect `fs.write`"`
   (`src/hir/validation/host_command.rs:20-25`, `require_effects`). No new
   diagnostic text — this is the same check every host-command operation
   already goes through, extended to one more operation identity.
2. **Non-exhaustive handling.** A caller that does not dispatch on all three
   codes `0`/`1`/`2` (for example, a boolean `== 0` check that silently
   folds `1` and `2` together) is not rejected by this operation's own
   admission rule — `usize` return values are not otherwise constrained by
   exhaustiveness in this language, and inventing an ad hoc exhaustiveness
   check for exactly one operation's return value would be a new admission
   rule with no precedent. Instead, `HOSTOUT-003` requires the **std wrapper**
   this document expects (`std.fs.write_atomic_checked`, analogous to
   `std.fs.write_atomic`) to return a real three-variant closed union/enum
   value once the language's existing closed-union facilities admit it, so a
   `match` over it is exhaustive by the verifier's ordinary exhaustiveness
   rule rather than by a bespoke special case for this one `usize`. Until
   then, the raw operation returns `usize` and callers are responsible for
   exhaustive dispatch the same way `std.jobs`'s lifecycle codes already are
   — this is a known, explicitly stated limitation of the first slice, not a
   silent gap.
3. **Hostile forgery.** A checked SEMAPRAX caller cannot supply `2` on input
   to *manufacture* an uncertain outcome — the return value is
   compiler-generated from the mapped provider status, never an argument the
   caller controls, so no new input-validation diagnostic is needed on the
   call itself. The hostile-input case this document requires instead is a
   **provider-conformance test**: a reference provider that reports
   `PublishUncertain` for a path that was never reached (i.e. before its
   commit step) must be treated as a provider defect to be caught by
   provider test scaffolding, not something the compiler can detect at
   compile time (the compiler has no visibility into the provider's internal
   commit boundary). This document records the requirement; it does not
   attempt to invent a compile-time check for a runtime provider's internal
   honesty, which is outside what a verifier can observe.

## Native and Wasm lowering (not implemented; the coordinated shape both must follow)

Both backends currently route every host-command status check through one
shared fail-stop mechanism:

- **Native**: `spx_status = <helper>(...); if (spx_status !=
  SPX_STATUS_SUCCESS) goto spx_epilogue;` (`src/codegen/native_emit/expression/host_command.rs:410`
  is the exact `FileWriteAtomic` site; `spx_epilogue` is defined once in
  `src/codegen/native_emit/mod.rs` and shared with arithmetic and contract
  fail-stops). `file_write_atomic_checked` must emit a **different** pattern
  at this one call site: the new helper (`spx_host_file_write_atomic_checked_v1`,
  a new native ABI symbol, not a reinterpretation of the existing one) must
  itself return the mapped three-way code rather than a pass/fail status, and
  the emitted C must bind that code directly to the expression's value with
  **no** `goto spx_epilogue` for this operation's own domain — a distinct
  native ABI failure (e.g. a null context, a capability check failing before
  the provider is even invoked) still fails closed exactly like every other
  operation's setup failure, since that is a different class of "did not
  happen" than the language-level taxonomy above.
- **Wasm**: `src/wasm/aggregate/host_command.rs` and `src/wasm/command_io.rs`
  encode status checks as inline comparisons against `self.plan.status`
  (e.g. `src/wasm/aggregate/host_command.rs:186-196`) that branch to the
  function's existing fail-stop exit depth
  (`self.control_depth + self.status_exit_extra_depth`,
  `src/wasm/aggregate/host_command.rs:337`). The new operation needs its own
  emission arm that skips that branch for its domain codes and instead
  writes the mapped `0`/`1`/`2` value to the result local, matching the
  native side's "no goto" rule bit for bit — this is exactly where the first
  non-negotiable invariant ("equivalent checked behavior on every backend
  that claims the admitted feature") is load-bearing: if native returns a
  value and Wasm still traps for the same provider condition, the profile is
  not admitted on Wasm and must not be marked as such.
- **Interpreter** (a third execution surface the repository treats as a
  reference lane, per `src/interpreter/`): needs the same three-way mapping
  in whichever host-command interpreter path currently short-circuits the
  whole evaluation on a nonzero provider result; this document does not cite
  an exact line because the interpreter dispatch point was not read in
  enough depth here to pin one, and an implementer must locate and cite it
  before lowering, not assume the native/Wasm sketch above transfers as-is.

None of the three lowerings above exist yet. This document fixes their target
shape; it does not attempt to write them, per this document's own Status line
and the review-checkpoint gate.

## Connection to `std.jobs`

Once `file_write_atomic_checked` (or any operation following this shape)
exists, a checked SEMAPRAX job handler can call it, receive `2` (`UNCERTAIN`),
and feed that directly as the `kind == 3` input to
`std.jobs.uncertain.retry_next_state_after_outcome` — closing exactly the gap
`DURABLE-JOBS-V1.md`'s "#228 boundary" section names as the tranche's one
remaining limitation. That wiring is `std.jobs` *composition* work for the
issue that lowers this document, not part of this document's own scope.

## What is and is not covered by this document

**Covered (design only):**
- The decision to add a value-typed outcome instead of exceptions
  (Decision 1) and to do so additively (Decision 2).
- The closed three-way taxonomy and its exact codes (`HOSTOUT-001`).
- Capability requirements (`HOSTOUT-002`).
- The three admission/diagnostic decisions (`HOSTOUT-003`), including the
  explicit limitation that exhaustive dispatch is not yet compiler-enforced
  for the raw `usize` shape.
- The target native and Wasm lowering shape, cited to the exact status-check
  sites both backends share today.

**Not covered / explicitly deferred:**
- Any implementation: no parser grammar, no HIR node, no verifier rule, no
  semantic graph projection, no native or Wasm codegen, no interpreter
  change. `HOSTOUT-003`'s exhaustiveness item is deliberately left as a known
  gap in the first slice rather than resolved here.
- The network family (`net_send`/`net_recv`/etc.) — named by issue #228 but
  intentionally out of the first slice (Decision 2).
- A general try/catch or exception language feature.
- `std.jobs` composition wiring — left to the issue that lowers this
  document.
- Admission into `docs/COMPLETION-MATRIX.md` — that file is coordinator-owned
  and this document records no status-row change; per the repository's own
  rule, no feature is "implemented" without the completion matrix's
  executable gate, and none of this is implemented.

## Tests added with this document (no lowering required)

`tests/useful_data/filesystem_v2_native.rs`,
`filesystem_v2_write_atomic_failure_collapses_into_undifferentiated_abort`:
compiles and runs generated C proving today's exact gap — a `write_atomic`
provider failure standing in for an "uncertain" condition produces the
identical `!semantic_success && status_code == 5` result and the identical
"no later operation runs" abort shape as an ordinary pre-publication
validation failure, with nothing in the generated carrier distinguishing
them. This is a regression against the *current* (pre-lowering) behavior:
the day `file_write_atomic_checked` lands, this specific test's assertions
are unaffected (it tests the unmodified `file_write_atomic`, not the new
operation), but the new operation's own success/`NOT_PUBLISHED`/`UNCERTAIN`
tests must all exist alongside it before that lowering can be considered
complete, per this document's `HOSTOUT-001` table and the repository's
"add a success case and stable diagnostic regression before or with the
implementation" change protocol rule.

## Open questions for the review checkpoint

- Should `HOSTOUT-001`'s taxonomy be a compiler-recognized closed union type
  (once the language has one general enough) rather than a bare `usize`, to
  make `HOSTOUT-003`'s exhaustiveness gap compile-time rather than
  documentation-only from the first slice?
- Should `PublishUncertain` require a distinct capability flag so a caller
  can statically know an operation is capable of returning `2`, versus
  inferring it from the operation identity alone (this document's Decision
  in `HOSTOUT-002` is "no", but a reviewer may weigh this differently)?
- Does the network family's `TransferFailed` case need its own document
  before or after this one lowers, and should it reuse `HOSTOUT-001`'s exact
  code assignment (`0`/`1`/`2`) or mint its own?
