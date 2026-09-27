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
- Migration of an Agent lifecycle example onto this mechanism: assessed and
  currently blocked; see
  [section 12](#12-agent-lifecycle-migration-assessment-issue-296).
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
non-containing branch refused rather than joined -- except that this
refusal is scoped to a genuine owned-value join: a function whose built
`CleanupPlan::slots` is empty (no owned `Bytes` local anywhere, so nothing a
join across the branch could lose track of) returns the vacuously empty
result immediately, without walking toward the site at all, regardless of
how many preceding statements branch on their own (bug #296, R20). Every
control-dependent site of a purely scalar (Copy-only) `yields` function is
therefore always admitted; the refusal above is only ever reached for a
function that already carries at least one owned slot.

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

## 12. Agent lifecycle migration assessment (issue #296)

Every candidate Agent lifecycle example in this repository was checked
against sections 1 and 11 above. None fits the admitted profile today without
changing its observable interface, so none was migrated; this section
records exactly which construct blocks each one, per the "stop and report"
instruction rather than forcing a fit.

Candidates considered:

- `examples/everyday-agent-project/src/agent.spx` (`agent EverydayAgent`) and
  `examples/offline-repair-project/src/app.spx` (`agent FixtureAgent`): the
  two real `agent { }` declarations in this repository, each with a
  `runtime_v1` models/tools/policy block and the fixed six-role
  `initialize`/`observe`/`propose`/`authorize`/`execute`/`reduce` operation
  set. `FixtureAgent` additionally declares its own `Step` variant
  (`Continue`/`Complete`/`Suspend`/`Fail`), a hand-authored duplicate of the
  exact vocabulary `resumable_effects::core::Step` generalizes -- the
  clearest instance in this repository of "a stage that waits for a
  model/tool answer via a bespoke state machine" the backlog item describes.
- `std/agent/src/agent.spx`: a plain, non-Agent-declaring stdlib package
  (`initialize`/`observe`/`advance`/`stage_transition`/`retry_admitted`/
  `retry_delay`). It has no suspension point at all -- every function
  returns immediately from its own scalar/record arithmetic, and
  `retry_admitted`/`retry_delay` are pure backoff-policy predicates a caller
  could use around any wait, not a wait themselves. There is nothing here to
  migrate.

What blocks the two real Agent declarations, precisely:

- **The request/answer channel type.** At this assessment's baseline, section 1
  admitted only Copy-scalar parameters, requests, answers, and results; section
  11.6 admitted one whole owned `Bytes` *local* across a suspension. Section
  12.1 and the R20 Bytes-leaf boundary below describe later bounded aggregate
  request admission and its remaining response/whole-function limits. The
  fixture's `Observation` and `Proposal` are flat Copy aggregates, so a model
  wait can use that channel shape once whole-function signatures and Agent
  binding admit it. `Outcome` includes owned `Bytes`, while `Task`, `State`
  and `Report` still require broader owned-state support. The bounded request
  leaf admission alone therefore does not migrate the Agent lifecycle.
- **Agent operations are not free, `yields`-eligible functions.** `yields` is
  a clause on an ordinary top-level `fn` (`parser::yields`,
  `hir::resolve_yield`), resolved and lowered by
  `resumable_effects::lowering`/`interpreter::resumable` directly. An Agent's
  `operations { }` block instead binds already-declared `fn`s by `@id` into
  one closed `semaprax.agent-definition.v1` document that
  `agent_lifecycle`/`project::compile_source_agent_declaration` compiles
  through its own, separate pipeline (see
  `src/agent_lifecycle/source.rs`). Adding a `yields` clause to a role
  function would not connect it to that pipeline; it would just make that
  same function additionally fail the current whole-function Copy-scalar
  profile when its parameter or result is an aggregate role.
- **The role that actually waits on a model declares an effect.**
  `propose` is a `model fn` and `execute` is an `effect fn` -- both cross a
  real host boundary today. `SPX-T302` refuses any `uses`-effect function a
  `yields` clause at all, precisely because this profile's resume replays
  the suspended prefix and would redispatch a real effect a second time.
  This is not itself the blocking construct for the *shape* of suspension
  (the continuation model already puts the actual host dispatch outside the
  checked function, at the durable driver's `dispatch`/`answer` boundary,
  section 3), but it means an Agent's own role signature cannot simply grow
  a `yields` clause in place; it would need a distinct, effect-free
  `yields`-declaring function whose request/answer *is* one of the fixed
  roles above, which section 1's admitted profile already refuses on typing
  grounds regardless.

None of this contradicts [Resumable Effects v1](RESUMABLE-EFFECTS-V1.md)'s
own acceptance table, which already records "Agents can progressively reuse
the mechanism rather than remain a separate runtime island" as **Open**
because "migrating even one Agent fixture requires ... a broader state
profile than this private scalar plan provides." This assessment narrows
that open item to the exact fields and clauses involved, so the increment
that widens the admitted request/answer profile to a bounded record or
variant shape (or gives an Agent role function its own effect-free `yields`
seam) can check its work against a concrete target instead of re-deriving
it. No source under `examples/`, `std/agent/`, or `src/agent_lifecycle/` was
changed by this assessment.

### 12.1 Blocker (1): bounded Copy-scalar aggregates admitted end to end (issue #296 R20)

An earlier R20 slice widened the request/answer channel-type admission the
first bullet of §12 named, at the compiler-shared type check only
(`hir::resolve_yield`/`hir::validation`), to admit a `yields` request or
response type that is a **bounded, flat, non-recursive record or variant of
Copy scalars**, not only a bare Copy scalar. Coordinator review found that
state not mergeable: `resumable_effects::lowering`'s own independent scalar
re-check (`require_scalar_expression_tree`), every envelope schema
(`source_checkpoint` v1-v4), the durable journal, and
`interpreter::resumable`'s `ArgumentValue`/`ResumableScalar` request/answer
representation were all still scalar-only, so a function that cleared the
widened HIR admission was compiler-admitted but not runnable: attempting to
lower it failed closed only much later, with the unrelated generic
`SPX-H006` ("invalid resumable HIR plan") rather than a stable, shape-aware
diagnostic. A checked program must either run or be refused with a stable
diagnostic before it ever reaches lowering; "admitted here, `SPX-H006` at
lowering" violates that, so **that admission was reverted**, and
`hir::yield_aggregate` was kept as a pure design record -- the exact shape
and bound a later increment's admission (and matching runtime support)
would use -- rather than a live admission.

A later R20 increment closes blocker (1) for the **direct top-level
(sequential) `yield` placement**: every layer `require_scalar_expression_tree`
above found missing now exists, and `hir::resolve_yield` admits that shape for request leaves, together with
matching runtime support, in the same change. A Bytes response is refused at
HIR admission because the current suffix profile cannot expose it safely.

- **Shape and bounds.** Unchanged from the design record: a record's own
  fields, or a variant's own case fields, must themselves be
  `hir::is_scalar_resolved_type` Copy scalars, never another record or
  variant (non-recursive by construction, always fully `Copy`); at most 8
  fields on a record and at most 8 cases of at most 8 fields each on a
  variant (`hir::yield_aggregate::MAX_YIELD_AGGREGATE_FIELDS`/
  `MAX_YIELD_AGGREGATE_CASES`); up to 8 direct owned `Bytes` leaves are also
  admitted, each capped at 1024 bytes. Generic instantiations, `String`, and
  nested aggregates remain refused.
- **Placement: sequential only.** `hir::resolve_yield`
  (`resolve_yields_clause`/`finish_yields_admission`) now admits the shape
  above for a `yields` request or response type, but only when every
  `yield` in the function is a direct top-level statement or tail value of
  the function's own body block. A control-dependent placement (`yield`
  reachable only through `if`/`else` or `while`) keeps the dedicated,
  stable `SPX-T307` regardless of whether the shape fits the bound:
  `resumable_effects::lowering::control` has no aggregate-channel runtime
  support, only the pre-existing owned-`Bytes`-carrying Copy-scalar profile
  (§11.6). A shape that does not fit the bound at all (too many
  fields/cases, a nested aggregate, or a generic instantiation) keeps
  `SPX-T307` unconditionally, independent of
  placement. `hir::validation`'s iterative validator re-derives the same
  admission at its own `Frame::Yield` (never trusting `resolve_yield`'s own
  work silently), and `resumable_effects::lowering`'s
  `require_scalar_expression_tree` and
  `lowering::control::check_resumable_profile`'s `allow_aggregate` gate
  re-derive it a third and fourth time, independently, exactly matching
  this repository's existing "every layer re-checks, never trusts" pattern
  for the Copy-scalar profile. Every pre-existing refusal for a
  non-aggregate type is unchanged: a borrow keeps `SPX-T305`, a resource
  keeps `SPX-T306`, and any other non-scalar, non-record/variant type
  keeps `SPX-T301`. A record/variant used as an ordinary intermediate value
  unrelated to the channel (not *exactly* the declared request or response
  type) keeps the original, unconditional `SPX-T303`; `check_scalar` widens
  only to admit a value of exactly the declared channel type anywhere in
  the body (the request built at a `yield` site, the bound answer, and any
  later expression over that same value, such as a field read's receiver),
  never any other aggregate.
- **Lowering.** `resumable_effects::lowering::ResumableScalar` gained
  `Record(Vec<ResumableScalar>)` and `Variant { case: String, fields:
  Vec<ResumableScalar> }` variants (canonical declared field order), used
  by the existing domain-separated suspension-binding hash
  (`hash_scalar`) unchanged in shape -- a bit-exact commitment to the exact
  request/answer history, now including an aggregate one, with no new hash
  domain needed.
- **Interpreter.** `interpreter::resumable::ResumableChannelValue` (`Scalar
  (ArgumentValue)` | `Record { declaration, fields }` | `Variant
  { declaration, case, fields }`) is a new, narrow boundary type, **not** a
  widening of `ArgumentValue` itself (which stays exactly as before and
  keeps every other interpreter lane's existing match arms exhaustive and
  unchanged). `interpreter::resumable::channel` is the new, parallel
  sequential-channel API (`run_sequential_channel_resumable_effect`/
  `resume_sequential_channel_resumable_effect`/
  `ResumableChannelContinuation`/`SequentialChannelResumableStep`), built by
  reusing the existing `Resumption`/`settle_yield`/`run_worker` state
  machine unchanged (it already carried the interpreter's own `Value`,
  which already represents a record or variant) and adding only the
  boundary conversions (`channel_of`, `value_of_channel`,
  `channel_to_resumable_scalar`) that were missing. The function's own
  declared parameters and overall return type stay Copy-scalar always --
  this increment widens only the `yields` channel, never a function's
  ordinary signature. Two narrow, pre-existing interpreter admission gates
  needed a matching widening for a fully Copy aggregate specifically
  (`interpreter::nested_owned`'s `bc_construct`/`bc_match`/`construct_ok`/
  `variant_ok`/`bc_bind_fields`, independent of that module's owned-variant
  transfer/aliasing profile, which stays unchanged): constructing a variant
  request/answer inline, and matching a variant answer in the suffix, both
  by value rather than through the owned-byte `Own`/`Borrow` machinery.
- **Non-durable checkpoint envelope.** `resumable_effects::source_checkpoint`
  gained `v5` for scalar-only aggregates and `v6` for an aggregate whose
  checked request shape has a `Bytes` leaf (`resumable_effects::source_checkpoint::channel`,
  wrapping a new `interpreter::resumable::checkpoint::encode_channel`/
  `decode_channel` inner codec), signature-bound like `v2`. Schema selection
  is from the checked signature; source admission currently refuses a `Bytes`
  response, and `v5` and `v6` never cross-decode. Encoding a scalar-channel
  continuation into either is refused
  (`SourceCheckpointError::ProgramMismatch`), and no existing code path in
  `v1`-`v4` was touched. `channel_json`/`channel_from_json` render a
  `Scalar` value byte-for-byte identically to the pre-existing
  `scalar_json`/`scalar_from_json` (the same `"tag"` values a bare scalar
  always used); `"record"`/`"variant"` are new tag values only a genuinely
  aggregate value ever produces, so every existing `v1`-`v4` fixture and
  the durable journal's own pre-existing scalar records stay byte-identical.
- **Durable journal.** `resumable_effects::continuation`'s `Carrier` gained
  a third variant, `SequentialChannel(ResumableChannelContinuation)`
  (never paired with the control-dependent lane, for the placement reason
  above). `ContinuationRequest.request`/`ContinuationAnswer.value` and the
  journal's own `Record::Answered.answer` widened from `ArgumentValue` to
  `ResumableChannelValue` -- again byte-identical for a `Scalar` answer, so
  every pre-existing scalar and control-dependent journal fixture is
  unaffected -- and `DurableInvocation::drive`'s injected handler is now
  `EffectHandler<ResumableChannelValue, ResumableChannelValue>`.
  `ContinuationRequest::bind_answer` takes `impl Into<ResumableChannelValue>`
  so an existing scalar-channel caller passing a bare `ArgumentValue` keeps
  compiling unchanged. The function's own overall result
  (`DurableOutcome::Completed`) stays `ArgumentValue`, matching the
  interpreter boundary's own scalar-return-type choice above.
- **Tests.** `interpreter::resumable::channel::tests` runs a record and a
  variant channel to completion on the interpreter directly (including a
  variant request constructed inline and a wrong-answer-type refusal, both
  `SPX-F113`/`SPX-F115`); `resumable_effects::source_checkpoint::channel::
  tests` round-trips `v5`, refuses a scalar-channel function
  (`ProgramMismatch`), and includes the negative control: a freshly,
  validly re-signed `v5` document with one field dropped from an encoded
  record still fails the round trip, isolating `decode_channel`'s own
  structural check from the outer HMAC; `resumable_effects::continuation::
  tests::channel` drives a record channel through the durable journal
  end to end, including a crash simulated before and after every one of
  its journal records, recovering with no repeated dispatch or cleanup,
  exactly like the pre-existing scalar and control-dependent crash
  matrices; `hir::resolve_yield::tests` covers the new admission (record and
  variant, sequential placement), the still-refused control-dependent
  placement, and every out-of-bound shape (field/case count, nesting,
  generics, an owned `Bytes` leaf) keeping `SPX-T307`.

Blocker (2) (Agent operations are not free, `yields`-eligible functions; the
role that actually waits on a model declares an effect) is entirely
untouched and remains exactly as described above. Also still open, exactly
as before: an owned `Bytes` leaf inside a bounded aggregate, and an
aggregate channel for the control-dependent placement (`resumable_effects::
lowering::control` would need its own aggregate-channel runtime support,
including the owned-`Bytes`-carrying combination that placement already
admits for a bare scalar channel).

### 12.2 Next slice: aggregate whole-function carrier

The existing channel lane deliberately ends at scalar ordinary parameters and
results. Widening only `lowering::control::check_resumable_profile` would be
unsound: the argument binding, suspension binding, completed value, durable
outcome, and journal replay would then disagree about the invocation's exact
meaning.

The smallest safe extension is a new sequential-only public entry point that
takes `&[ResumableChannelValue]` and returns a
`SequentialChannelResumableStep` whose `Completed` value is also a
`ResumableChannelValue`. It admits each parameter and result only when it is
an admitted scalar or `yield_aggregate::bounded_aggregate_refusal` succeeds;
all parameter modes remain by-value. The existing `ArgumentValue` APIs stay
unchanged.

Implementation must make these changes as one versioned carrier slice:

- `interpreter::resumable::channel`: convert each supplied channel value with
  the checked parameter type before evaluation; derive the suspension binding
  from its canonical `ResumableScalar` leaves; return the checked result by
  the same conversion. Replay compares every parameter, request, and answer
  bit exactly.
- `resumable_effects::continuation::{facts, DurableInvocation, lane}`:
  retain channel arguments and a channel completed value, and derive the
  `Started.arguments_digest` from canonical channel JSON, including nominal
  declaration and selected-case IDs. A scalar argument must retain its
  existing digest bytes.
- `resumable_effects::continuation::journal`: introduce a new journal schema
  identity for aggregate `Started` arguments and aggregate `Completed`
  values. It must never reinterpret an existing scalar journal; recovery
  chooses the schema from the checked signature before accepting bytes.
- `resumable_effects::source_checkpoint`: add a distinct envelope version for
  aggregate invocation arguments and results. The version binds the same
  signature, scope, plan, argument history, and scalar-leaf bit pattern as
  the live carrier.

Required tests are: record and variant parameter/result success through two
sequential yields; wrong nominal ID, case ID, field count, and scalar leaf
refusals before dispatch; source and graph canonical round trips; replay with
a changed aggregate argument; and a crash before and after every aggregate
`Started`, `Answered`, and `Completed` record, proving no second dispatch or
cleanup. Only after those pass may the Agent bridge bind FixtureAgent's
effect-free checked `propose` wait: it must dispatch through
`DurableInvocation::dispatch` exactly once, consume the ordinary model grant
there, and preserve the existing cancellation, replay, and refusal paths.

## 13. Assurance and conformance evidence for the admitted profile

What is proved, and how, for both admitted `.spx` lanes -- sequential
(v1/v2) and control-dependent (v3/v4) -- as of this increment:

- **Compiler-checked admission is the primary proof**, not a downstream
  test: `SPX-T297`/`SPX-T299`/`SPX-T301`/`SPX-T302`/`SPX-T303`/`SPX-T305`/
  `SPX-T306` (section 11.5) reject every source shape outside the profile
  before lowering ever runs, and `resumable_effects::lowering`/
  `lowering::control` independently re-derive plan identity, state
  identities, and (for a carrying control plan) each site's carried-locals
  list rather than trusting authored labels.
- **Runtime-guarded contract placement.** `src/assurance_manifest/resumable.rs`
  binds every `requires`/`ensures` clause of a `yields`-declaring function to
  the exact compiler-owned transition it runs across (`start`: entry to the
  first suspension; `final_resume`: the last suspension to completion),
  keyed to that function's own plan identity digest. This increment extends
  it from the sequential lane only to both lanes: `is_control_dependent`
  selects `lower_control` over `lower_sequential` the same way
  `source_signature::derive_source_effect_signature` already does, so a
  control-dependent function's contracts are bound and reported exactly like
  a sequential one's, with its own `bounds` string recording the dynamic
  suspension count and whether the plan carries an owned `Bytes` local
  (`control_dependent_copy_scalar_yields:<n>[:carries_owned_bytes]`). Every
  recorded method stays `AssuranceClass::RuntimeGuarded` with
  `runtime_fallback: true` -- a checked placement backed by the interpreter's
  own re-execution at every resume, never a static proof of the contract
  itself. The sequential lane's method additionally names a `target`
  (`resumable_yield_free_projection`, the one backend projection
  `resumable_effects::backend`'s `cfg(test)`-only parity runners actually
  execute); the control-dependent lane's method leaves `target` absent,
  because no yield-free projection exists for it at all (`SPX-H006`) --
  absence here is itself evidence, not an omission, of exactly what this
  profile does not yet prove.
- **Interpreter-only, by construction.** Every method this manifest records
  for a `yields` function is `RuntimeGuarded`, and the sequential lane is the
  only one with any target evidence, itself `cfg(test)`-only native/Wasm
  *parity* against the interpreter's own semantics rather than a production
  target claim (section "Compiler-owned lowering..." of
  [Resumable Effects v1](RESUMABLE-EFFECTS-V1.md)). Ordinary native
  (`SPX-B116`) and Wasm (`SPX-W126`) emission still refuse every `yields`
  function outright, sequential or control-dependent, carrying or not. There
  is no target evidence, hosted evidence, or production claim for the
  control-dependent or owned-`Bytes`-carrying profile at any layer.
- **What is tested, concretely.** Section 10's gate, plus
  `cargo test --locked -p semaprax --lib assurance_manifest::resumable::` for
  the contract-placement obligations above (both lanes: entry/first-site and
  last-site/complete binding, the `bounds` string, source-drift detection
  under unchanged contract ids, and the sequential lane's existing one-site
  and unsupported-projection refusals). None of this is hosted or is a fresh
  full test run beyond what each gate command itself performs.

Migrating an Agent lifecycle example onto this mechanism (section 12) would
add its own conformance oracle -- the example's existing tests passing
unchanged, plus a crash/restart test through the durable journal -- once the
request/answer profile or the Agent/`yields` seam above is widened enough to
attempt it; this section records only what the admitted profile itself
proves today, which does not yet include that.

#### R20 Bytes-leaf boundary

The v6 channel codec and signature derive the schema from the checked request
shape so a stored envelope cannot be interpreted under the wrong
schema. The presently executable source profile is narrower: an inline,
direct sequential request may contain bounded `Bytes` leaves only when no
owned local is live before the yield, and only one site is admitted. A v6
inner carrier and outer envelope each have a separate 64 KiB cap, sufficient
for eight 1 KiB leaves rendered as decimal JSON bytes in that one request.
HIR validation and workspace relinking independently recheck this one-site
bound before execution or checkpoint recovery.
A declared `Bytes` response that reaches
the current suffix lowering remains refused; it is not evidence of response
side support. Borrowed views remain `SPX-T305`, named owned locals before the
yield remain `SPX-T303`, and control-dependent aggregate channels remain
`SPX-T307`. Two sequential `Bytes` requests are refused at HIR admission with
`SPX-T307`; the one-site v6 round trip does not establish fresh request
allocation after replay history.
