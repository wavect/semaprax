# Resumable effects continuation v1

Status: **local library contract, second slice.** Implemented for the
sequential Copy-scalar profile and the control-dependent profile of section 11
by
`src/resumable_effects/continuation.rs` and its `journal` submodule (Unix
only). It is not a CLI, service, hosted, or production runtime, and it adds no
language syntax.

Audience: compiler and runtime contributors, host integrators embedding the
library, and reviewers auditing issue #296.

Owner: kevin.riedl@wavect.io. Contract identity:
`semaprax.resumable-continuation.v1`; journal line schema:
`semaprax.resumable-journal.v1`. Any breaking change to either needs a new
identity. [Resumable Effects v1](RESUMABLE-EFFECTS-V1.md) still owns the source
profile, lowering, envelope, and backend-refusal rules this document builds on.

## 1. Profile

The contract admits exactly the functions that the v2 source checkpoint
envelope admits: an explicitly identified free function with **two to eight**
direct sequential top-level `yield` sites, Copy-scalar parameters, request,
answer, and result, and no ordinary effects. A one-site function is refused
with `UnsupportedProfile` because the sequential continuation carrier does not
exist for it. Admission, the budget check (`1..=MAX_STEPS_LIMIT` interpreter
steps per segment), and the pure start plan run **before** any file is
created, so a refused start leaves storage untouched.

Section 11 adds the control-dependent profile (yields inside `if`/`else`
branches and `while` bodies). The lane is chosen by the checked program's
plan, never by stored bytes.

Execution uses only the compiler-owned start and per-site resume plans through
`interpreter::resumable`. The driver adds no second evaluator: every resumed
segment replays the recorded prefix and checks each historical request bit for
bit, exactly as the in-memory lane does.

## 2. Identities

| Name | Definition |
| --- | --- |
| program digest | `SourceEffectSignature::plan_identity`, the sequential lowering identity of the exact checked program and selected function |
| invocation id | caller-chosen, 1..1024 bytes, no control characters |
| policy epoch | caller-chosen `u64` |
| site index | number of settled sites before this suspension (`0..yield_count`) |
| envelope | the v2 HMAC source checkpoint of the suspended continuation, scoped to `resumable-plan:sha256:<program digest>`, the invocation id and the policy epoch |
| envelope digest | SHA-256 of the exact envelope bytes |
| arguments digest | SHA-256 over the canonical scalar JSON of each argument, each followed by a newline |

## 3. Non-bearer request and answer

A `ContinuationRequest` is `{program digest, invocation id, site, envelope
digest, request scalar}`. A `ContinuationAnswer` carries the same four identity
fields and one scalar value. The request is a description; holding it, or the
envelope, or the journal, confers nothing.

`answer` accepts an answer only when all of the following hold, checked in this
order, and otherwise refuses with the named class and leaves the journal
byte-identical:

1. The invocation is awaiting an answer (`Dispatched` is the tail). A settled or
   terminal invocation refuses `AlreadySettled`; an undispatched site refuses
   `NotDispatched`, or `ReplayedAnswer` if the answer names an earlier site.
2. The host presents a `CapabilityPolicy` that allows the selected function's
   persistent id (`CapabilityDenied`).
3. The program digest matches (`ProgramMismatch`).
4. The invocation id matches (`InvocationMismatch`).
5. The site is the awaited one: an earlier site is `ReplayedAnswer`, any other
   is `SiteMismatch`.
6. The envelope digest is the awaited one (`StaleEnvelope`).
7. The value has the compiler-derived answer type (`AnswerTypeMismatch`).

Consequently an answer is single-use: once its `Answered` record is durable the
same answer can only ever be replayed, and a replay is refused both in the live
process and after recovery.

## 4. Host authority

The continuation carries no authority. The library never opens storage by
itself: the host opens one owner-private directory and passes it as a
`JournalDirectory`. The host supplies its HMAC key by reference and its
`CapabilityPolicy` at every `dispatch`, `answer`, and `abandon`. Decoding a
genuine envelope or journal dispatches nothing, answers nothing, and evaluates
nothing that is not already determined by durable records. The HMAC key only
authenticates bytes; it is never serialized and is zeroized on drop.

## 5. Settlement

Terminal outcomes are sticky: `Completed(result)` or `Failed(class)` with
classes `language_failure`, `fuel_exhausted`, `call_depth_exceeded`,
`evaluation_rejected`, `handler_failed`, `answer_type_mismatch` and
`host_abandoned`. After the terminal record, terminal cleanup runs through the
host's `CleanupHandler` at most once. A cleanup failure is recorded as its own
settlement (`failed`) and never replaces the outcome. The outcome is
**published** (`ContinuationStatus::Settled`) only after the cleanup settlement
is durable; before that the status is `CleanupPending` or `CleanupInDoubt`,
which carry at most the failure class and never a completed result.

## 6. Journal protocol

One journal file per invocation, named by a domain-separated SHA-256 of the
invocation id plus `.journal`, so no caller text becomes a path component.

Storage rules:

- The directory must be a directory owned by the effective user with no group
  or other permission bits; it is opened with `O_DIRECTORY | O_NOFOLLOW`.
- Journals are opened relative to that descriptor with `openat` and
  `O_NOFOLLOW`; a journal is created with `O_CREAT | O_EXCL` and mode `0600`,
  and the directory is flushed after creation (`F_FULLFSYNC` on Apple
  platforms, falling back to `fsync`). A second start of the same invocation
  refuses `AlreadyStarted`.
- Each open journal holds an exclusive non-blocking `flock` for the lifetime of
  its descriptor, taken before any byte is read. A second live instance, in
  this or another process and including a poisoned instance not yet dropped,
  refuses `JournalBusy`; this is the single-writer rule that keeps concurrent
  recoveries from dispatching or cleaning up twice.
- A journal must be a regular file owned by the effective user, mode without
  group or other bits, with exactly one link.
- Each record is one line passed to a single `write_all` on an `O_APPEND`
  descriptor (which may issue several `write` calls), then `sync_all`ed
  (`F_FULLFSYNC` on Apple platforms). The append is **acknowledged** only when
  both succeed; any later step may rely only on acknowledged records. Any
  error after an acknowledged append, from storage or from preparing the next
  record, poisons the in-memory invocation (`Poisoned`); only recovery may
  continue it.
- If `sync_all` fails after the write, the record may or may not be durable.
  The invocation is poisoned and recovery acts on whatever is observed. For
  `Dispatched` and `CleanupStarted` this is exactly the in-doubt case: the
  handler has not run, recovery sees either the previous tail (the step is
  retried, never having happened) or the new record (in doubt, never
  repeated).

Each line is canonical JSON
`{"mac","prev","record","schema","seq"}`, where `seq` counts from zero, `prev`
is `sha256:` of the previous line's bytes (all zero for the first), and `mac`
is `hmac-sha256:` over the domain `semaprax.resumable-journal-record.v1\0` and
the canonical JSON of the other four fields. A journal holds at most 28 records
and 512 KiB. `Started` also binds the per-segment step budget (`max_steps`),
so recovery under a different budget refuses `BudgetMismatch` instead of
reaching a different terminal outcome.

Records, in the only admitted order:

```text
Started{contract, function, program_digest, invocation_id, policy_epoch,
        arguments_digest, yield_count, max_steps}
( Yielded{k, envelope, envelope_digest}
  Dispatched{k, envelope_digest}
  Answered{k, envelope_digest, answer, answer_digest} )*    k = 0, 1, ... in order
then exactly one of:
  Completed{result} | Failed{language_failure | fuel_exhausted |
                             call_depth_exceeded | evaluation_rejected}
      -- after Started or Answered (a plan step)
  Yielded{k} Dispatched{k} Failed{handler_failed | answer_type_mismatch |
                                  host_abandoned}
      -- a dispatched site settled as a host failure
CleanupStarted
CleanupSettled{completed | failed | host_confirmed}
```

Any prefix of this sequence is a valid observed journal.

Ordering obligations of the writer:

- `Yielded` is durable before a request can be dispatched.
- `Dispatched` is durable before the request is returned to the host or passed
  to its handler.
- `Answered` is durable before the resume plan runs.
- The terminal record is durable before cleanup starts; `CleanupStarted` is
  durable before the cleanup handler runs.

## 7. Recovery

`recover` requires the same checked program, function, arguments, invocation
id, policy epoch, key and budget, reads the whole observed journal, and:

1. Verifies every complete line: exact field set, schema, sequence, chain,
   authentication, and byte-for-byte canonical form. Any failure is
   `TamperedJournal` (or `SchemaMismatch`); nothing is repaired or skipped.
2. Only after every complete line verifies, considers a final fragment with no
   trailing newline. Such bytes were never acknowledged. Under
   `TornTailPolicy::Refuse` recovery refuses `TornTail` and changes nothing;
   under `TornTailPolicy::TruncateUnacknowledged` it truncates exactly the
   bytes after the last newline and `fsync`s. A newline-terminated line is
   never truncated, whatever its content. A torn write that persisted later
   pages but not earlier ones can leave a newline-terminated but corrupt line:
   that fails closed as `TamperedJournal`, and v1 has no recovery path for it.
3. Compares `Started` with the caller's facts: `InvocationMismatch`,
   `FunctionMismatch`, `ProgramMismatch` (digest or yield count),
   `PolicyEpochMismatch`, `ArgumentsMismatch`, `BudgetMismatch`.
4. Replays records through the order above; out-of-order, wrong-site or
   wrong-digest records are `TamperedJournal`. Each `Yielded` envelope is
   decoded through the v2 envelope decoder under the recovered scope.
5. Continues from the observed tail:

| Tail | Recovery action |
| --- | --- |
| none (empty after torn-tail truncation) | acknowledge `Started` from the caller's facts, run the start plan |
| `Started` | run the pure start plan, append its step |
| `Yielded` | `AwaitingDispatch`: the site may be dispatched (it never was) |
| `Dispatched` | `AwaitingAnswer{in_doubt: true}`: never dispatched again; only an explicit bound answer or `abandon` settles it |
| `Answered` | run the pure resume plan with the journaled answer, append its step |
| terminal | `CleanupPending`: cleanup may run (it never started) |
| `CleanupStarted` | `CleanupInDoubt`: cleanup is never run again; only `confirm_cleanup` settles it (`host_confirmed`) |
| `CleanupSettled` | `Settled`: nothing further happens |

These rules give the two guarantees of this contract: **a settled yield is
never dispatched again** and **cleanup never runs twice**, across any crash
that preserves acknowledged records.

## 8. Threat model and limits

Covered: crashes at append boundaries (the tests simulate a crash immediately
before and after each append), torn final appends, tampered, reordered, or
dropped middle records, answers replayed or rebound across program, invocation,
site, or envelope, journals from another invocation or key, symlinked or
foreign directories and journals, and hosts without the capability.

Not covered in v1: an adversary who can write the owner-private directory and
**removes whole acknowledged trailing records** (rollback) is not detected;
that needs an external monotonic counter. `flock` is advisory: a process that
writes the file without taking the lock is outside the model. Ownership and
mode bits are checked, but macOS ACLs are not detected. Non-Unix hosts do not
compile the module.

## 9. Remaining work (issue #296)

- Owned values live across a yield: section 11.6 admits only a whole owned
  `Bytes` local in the control-dependent plan; owned live frames generally
  (strings, records, partial/conditional liveness, the sequential plan) stay
  future work.
- Ordinary native and Wasm emission of `yields` functions (`SPX-B116` and
  `SPX-W126` still refuse) and a durable driver for those engines.
- Migration of an Agent lifecycle example onto this mechanism.
- One-site functions, rollback detection, a CLI or service surface, and
  hosted evidence.

## 10. Gate

```sh
cargo test --locked -p semaprax --lib resumable_effects::continuation::
cargo test --locked -p semaprax --lib interpreter::resumable::control::
cargo test --locked -p semaprax --lib resumable_effects::lowering::control_tests::
cargo test --locked -p semaprax --lib resumable_effects::source_checkpoint::control::
cargo test --locked -p semaprax --lib resumable_effects::source_checkpoint::control_owned::
cargo test --locked -p semaprax --lib parser::yields:: hir::resolve_yield::
```

This runs the positive multi-yield run, the explicit request/answer exchange,
sticky host failure, the crash before and after each of the thirteen records of
a three-site run, and the hostile answer, fact, journal, torn-tail, directory,
and admission cases. These are local, offline results, not hosted evidence.

## 11. Control-dependent profile (issue #296, slice 2)

### 11.1 Admitted placements

A `yield` may now also be the direct `let` or assignment value of a block
reached from the function body only through `if`/`else` branches, `while`
bodies, or block-valued slots, at any depth. It stays refused (`SPX-T297`) in
conditions, operands, call arguments, a nested block's tail, `match` arms,
`for` and `unsafe` bodies, and closures. The function's own tail may still be a
`yield`. Parser (`parser::yields`), resolver (`hir::resolve_yield`, including
the while-body admission scan), HIR validation, canonical formatter and
semantic graph carry the new placements; tests assert the canonical round-trip
and the graph's `yield` nodes.

### 11.2 Plan and continuation (identity v3)

`resumable_effects::lowering::control` lowers such a function to a
`ControlResumablePlan`: one static suspended state per `yield` site (at most
eight), with plan identity domain `semaprax.resumable-control-plan.v3`. A
function whose yields are all direct top-level sites is refused here and keeps
its sequential plan, so the two identities never coincide.

The dynamic control state at a suspension is the ordered list of settled
suspensions, each `(static site, request, answer)`. The continuation binding
(domain `semaprax.resumable-control-binding.v3`) commits to the plan identity,
the current site, the exact scalar arguments, and every settled
`(site, answer)` pair. Resume replays the pure prefix from entry: each replayed
suspension must occur at exactly its recorded site and recompute its recorded
request (`SPX-F114` otherwise), and the binding must match (`SPX-F115`). The
branch taken and the loop iteration reached are recomputed, never read from
the continuation.

Loops are bounded dynamically: at most 16 suspensions per invocation
(`MAX_CONTROL_SUSPENSIONS`). Reaching a 17th is the sticky terminal failure
`suspension_bound_exceeded`. The journal bound grows to 52 records
(1 + 3 x 16 + 1 + 2).

Section 1's per-segment step budget (`1..=MAX_STEPS_LIMIT`) still applies
unchanged to each resume of this profile: one interpreter worker call runs the
replayed prefix and the newly reached suffix together from entry, so a
replayed suspension's steps are not free. A control-dependent resume nearer
`MAX_CONTROL_SUSPENSIONS` therefore has fewer steps left over for genuinely new
work than an earlier one at the same `max_steps`, exactly as the sequential
profile's own replay already spends its segment budget.

### 11.3 Envelope v3

`semaprax.source-resumable-checkpoint.v3` is a new, separate wire with its own
authentication domain. It carries the v2 scope and signature facts (with
`"plan": "control"`) and the continuation's state, binding, request and settled
history with sites. Decode re-lowers the current program, maps every site to
the current plan, and re-derives the binding; v1 and v2 decoders refuse v3
bytes by schema and the v3 decoder refuses theirs. Nothing reinterprets v2
bytes.

### 11.4 Driver and journal

The durable driver selects the lane from the checked signature. For the control
profile the journal's `site` is the dynamic suspension ordinal (the number of
settled suspensions), so the non-bearer answer binding of section 3 applies
unchanged: an answer names the program digest, invocation, ordinal and the
exact v3 envelope digest. Recovery decodes `Yielded` envelopes as v3. Every
recovery rule of section 7 holds; the crash matrix covers a crash before and
after each of the 16 records of a loop-and-branch run.

### 11.5 Stable refusals

| Code | Refusal |
| --- | --- |
| `SPX-T297` | `yield` outside the admitted placements |
| `SPX-T302` | effectful prefix: a `yields` function declares `uses` effects, whose replay would redispatch them |
| `SPX-T303` | an owned or aggregate value that is not an admitted Copy scalar |
| `SPX-T305` | a borrow (`borrow T`, `str`, `Slice<u8>`) in a `yields` function, which could be live across a suspension |
| `SPX-T306` | a resource or handle in a `yields` function, which could be live across a suspension |
| `SPX-B116` / `SPX-W126` | ordinary native / Wasm emission of any `yields` function, including the new placements |
| `SPX-H006` | resumable target preparation of a control-dependent plan (no yield-free projection exists) |

`SPX-T305` and `SPX-T306` are checked for parameters and every intermediate
value; they are conservative (any borrow or resource in the function), not a
liveness analysis.

### 11.6 Owned values across a yield

Every owned value stays refused with `SPX-T303` (and every borrow/resource
keeps `SPX-T305`/`SPX-T306`, exactly as before) except a whole `Bytes` local
the narrower query below proves live across a real suspension site: strings
and owned records stay refused everywhere in a `yields` function regardless of
whether they ever reach a suspension, and a `Bytes` local that does reach one
still needs whole (not partial record-field or conditional-variant) storage
and a site the query does not itself refuse (a preceding statement branching
on its own, say). A `Bytes` local that never reaches a suspension at all --
a call's own transient storage, or a genuinely dead local -- needs none of
this: it resolves entirely within one non-suspended segment.

First increment: `cleanup_plan::owned_liveness::owned_locals_live_at` (issue
#296) implements, in isolation, the query one suspension site's `ExpressionId`
answers with the ordered subset of the built `CleanupPlan`'s `slots` still
live there, by replaying the plan's own `CleanupTransition`s restricted to the
prefix a structural walk of the HIR proves runs before the site, never a fresh
HIR move analysis and never a re-sort of `slots`. Its scope is deliberately
narrower than the general design a later slice may still generalize: only
whole-storage places (no partial record-field or conditional variant
liveness), and only sites reached through the same `if`/`else`/`while`/
block-valued nesting the control-dependent placements admit, with a preceding
non-containing branch refused rather than joined.

Second increment (this document's own contract): the query is un-gated from
`cfg(test)` and wired to a real caller on both ends.

- **Admission.** `hir::resolve_yield::check_scalar` defers exactly one case
  instead of refusing it immediately: an unborrowed value of type `Bytes`.
  Once every function's `cleanup_plan` is built,
  `cleanup_plan::admit_owned_bytes_profile` walks every `yields`-declaring
  function's plan and checks only the slots the query reports live at some
  real suspension site (the union across every site; a site the query itself
  refuses fails the whole function): each such slot must be an unborrowed,
  whole (`StorageId::Value`, leaf field-liveness shape) `Bytes` local, else
  `SPX-T303`, now raised here instead of at the deferred check. A slot no
  site ever reports live -- a call's own transient provisional-result or
  staged call-argument storage, or a genuinely dead local -- resolves
  entirely within one non-suspended segment and needs no such proof; a
  non-`Bytes` owned value never reaches a plan slot in the first place, since
  `check_scalar` only ever defers `Bytes`. This is a compile-time gate, not a
  runtime one: a program that fails it never reaches lowering.
  **A loop-embedded carried site admits exactly when the carried local's own
  storage is untouched inside the loop.** A `while` body can suspend more
  than once per invocation, and the carrying substitution (below) is a flat
  map keyed by the static `let` binding, consumed on the *first* dynamic
  occurrence a resume's replay reaches. That is sound exactly when the
  carried local's own storage (its `Initialize`/`Renew`/`Transfer`/
  `ReserveRenewal`/`CallCommit`-argument transitions) is never triggered
  *inside* a `while` body: such a local's own `let`/assignment then reaches
  exactly one dynamic occurrence per invocation regardless of how many times
  the loop-embedded site downstream suspends, so the one recorded value is
  the right one for every occurrence. `cleanup_plan::owned_liveness::
  slot_touched_inside_while` decides this per live slot -- an exhaustive
  structural walk (every expression kind via
  `hir::push_resolved_expression_children_in_authored_order`, every statement
  kind via `ResolvedStatement::child_count`/`child`, so a transition trigger
  nested in a `match` arm or an `unsafe` boundary inside a `while` body is
  found exactly like one nested in `if`/`else`, never missed by a narrower,
  placement-grammar-restricted walk) -- rather than blanket-refusing every
  live slot merely because the *site* sits inside a `while` body.
  `admit_owned_bytes_profile` refuses (`SPX-T303`) a loop-embedded site's
  live slot only when `slot_touched_inside_while` finds it touched inside the
  loop: a local whose own storage *is* touched there can legitimately hold a
  different value at each dynamic occurrence of its own binding, which the
  flat map cannot represent (only the first would ever be consulted), so
  that shape stays refused. In practice, `hir::resolve_statement`'s own
  pre-existing Bounded While-Loops v1 admission (`SPX-T252`) already refuses
  most ways of touching an owned `Bytes` local's storage inside a `while`
  body (no owned-Bytes-producing or non-scalar-returning call, no record
  construction, no method call), so `SPX-T303` here is a second line of
  defence rather than the first for source admitted today; the admitted
  shape that reaches lowering at all is a `Bytes` local defined -- and never
  reassigned -- *before* the loop, live across a suspension reached through
  it.
- **Lowering.** `resumable_effects::lowering::control::lower_control` (the
  control-dependent plan only; the sequential plan still refuses any owned
  cleanup state) computes each `ControlSite`'s `carried: Vec<ValueId>` from
  `cleanup_plan::carried_locals_at`, and the plan's own
  `carries_owned_bytes` is true when any site's list is non-empty. A carrying
  plan gets its own identity domains,
  `semaprax.resumable-control-plan.v4`/`semaprax.resumable-control-binding.v4`,
  distinct from the non-carrying v3 domains, so the two families can never
  collide even over identical hashed facts; `ControlResumablePlan::binding`
  additionally commits to the exact carried bytes at the site being bound.
- **Envelope.** `semaprax.source-resumable-checkpoint.v4`
  (`resumable_effects::source_checkpoint::control_owned`) is a new, separate
  wire with its own authentication domain, carrying v3's scope, signature, and
  continuation facts plus `carried`: each carried local's exact bytes, hex
  encoded, in the plan's own cleanup-inventory order for the awaited site.
  Every field, carried bytes included, is inside the HMAC-covered payload, so
  a tampered carried value is refused (`AuthenticationMismatch`) exactly like
  a tampered request or answer. v1/v2/v3 decoders refuse v4 bytes by schema
  and the v4 decoder refuses theirs; nothing reinterprets v2/v3 bytes. The
  durable driver's lane selection (`resumable_effects::continuation::lane`)
  picks v2, v3, or v4 purely from the checked
  `SourceEffectSignature::{is_control_dependent, carries_owned_bytes}`, never
  from stored bytes.
- **Interpreter resume.** The control lane's execution model is unchanged --
  one evaluator run replays the pure prefix from function entry and
  re-checks every recorded request -- so "receiving the carried value as
  input instead of recomputing the prefix" is a substitution inside that same
  replay rather than a second, statically projected HIR function (the control
  lane has no such projections to begin with; only the retired sequential
  lane's backend parity projections do). Concretely: `interpreter::resumable`
  snapshots the live frame (`Resumption::{Fresh,Replay}::parked_environment`)
  at the moment of park, and a control resume's `Resumption::Replay::carried`
  (`BTreeMap<ValueId, Value>`, built from the continuation's carried bytes)
  is consulted at every `let` statement: a binding named there is bound
  directly from the map and removed, never re-evaluating (and so never
  reallocating) its own defining expression. Only the resumed segment's own
  carried locals are substituted this way; an earlier settled segment's own
  locals, if any, are still recomputed during replay -- deterministically,
  since the profile forbids host effects, but not yet avoided the way this
  section's design intends for a future, more general slice.
- **Settlement.** No second window: the existing journaled
  `CleanupStarted`/`CleanupSettled` window is reused exactly as it is, and
  `CleanupHandler<Op>` (`resumable_effects::core`) is extended, not
  replaced: alongside its existing `run` (the overall sticky outcome, still
  settled exactly once, unchanged), a new `run_carried(&mut self, item:
  &[u8]) -> Result<(), String>` settles one carried owned value, called once
  per entry of `DurableInvocation::pending_cleanup_carried` in that vector's
  own order -- the same per-op audit shape `resumable_effects::core::resume`
  already gives its own `Vec<(CleanupOp, Result<(), String>)>`, rather than
  one outcome folded silently over several operations. `run_carried`'s
  default fails closed (`Err`), so a handler written before this profile
  ever had something to carry does not silently claim settlement for bytes
  it never touches. `pending_cleanup_carried` is non-empty only for the
  three failures recorded while a site was dispatched (`HandlerFailed`,
  `AnswerTypeMismatch`, `HostAbandoned` -- `DurableFailure::settles_dispatch`):
  those are the only outcomes where the interpreter never re-ran to its own
  natural, in-process drop of the carried value. A `Completed` outcome, or
  any other `Failed` one, only ever follows a resume or start that ran the
  interpreter through to that outcome, which already dropped every carried
  value itself, so `pending_cleanup_carried` stays empty for those.
  `DurableInvocation::settle` calls every carried item's own `run_carried`
  -- none skipped by an earlier item's failure -- and reports
  `CleanupSettlement::Completed` only when `run` and every `run_carried`
  call succeeded; any failure, including a handler that never overrides
  `run_carried`, reports `Failed` instead, in both `status()` and the
  durable journal's own `CleanupSettled` record. The driver itself, not a
  caller-owned counter, is what makes a skipped carried settlement visible.

Scope carried over unchanged from the first increment, and still true of the
second: only whole-storage `Bytes` locals, only the control-dependent plan,
only the placements the increment 1 grammar note above already lists, and --
new to this increment -- a loop-embedded carried site admits when the
carried local's own storage is untouched inside the loop (in practice, a
`Bytes` local defined and never reassigned before the loop); a carried local
whose own storage is touched inside the loop stays refused (`SPX-T303`),
since the flat, `ValueId`-keyed carrying substitution cannot soundly
represent a value that legitimately differs across that binding's own
dynamic occurrences. Making the substitution dynamic-occurrence-aware, to
admit that broader shape too, remains future work.
