# Bounded SMT Discharge v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, plus compiler contributors working on
contract verification, the Assurance Manifest v1 obligation join (#129,
#183), and the later model-checking (#185) and proof-kernel (#186) backends.

Bounded SMT Discharge v1 (`src/assurance_manifest/smt_discharge/`) is the
first real static proof backend for a deliberately small, closed, pure
subset of SEMAPRAX contracts: integer/boolean arithmetic without division,
comparisons, `and`/`or`/`not`, `if`, and immutable `let`, over
`{i64, i32, u8, usize, bool}`. It translates one function's `requires`,
`ensures`, and body into a `QF_LIA` SMT-LIB2 query, runs it through an
explicitly provisioned solver under bounded time and output, validates any
`sat` model by independently replaying it against checked-arithmetic
semantics, and classifies the result as proved, a validated concrete
counterexample, or one of several distinct non-result outcomes. It is proof
data, not permission: it never runs a target, discovers or runs project
tests, writes source, or removes a runtime guard.

## Why this exists

Issue #184 asks for a useful bounded subset to receive genuine SMT proof or
concrete counterexamples, with `unknown`/timeout/unsupported never reported
as proved, solver semantics matching SEMAPRAX's runtime numeric semantics,
reproducibility under a pinned explicit toolchain, and an explicit, tested
runtime-guard fallback. Every one of those constraints shaped a design
decision below; see "Explicitly deferred" for what a useful, honest v1
narrows away rather than ships unverified.

## Supported subset

`subset::check_declaration_supported` and `translate::translate_function`
jointly define the admitted grammar. A function is discharged wholesale or
not at all: this tranche never discharges half a function's clauses while
silently skipping the unsupported half, so a caller cannot end up trusting a
partial translation it did not ask for.

Admitted:

- Types: `i64`, `i32`, `u8`, `usize`, `bool` for every parameter, and (only
  when at least one `ensures` clause exists) the return type.
- Literals of those five types, including the `usize` truncated literal and
  the `i64`/`i32` extrema.
- `Var` referring to a parameter, an immutable `let` in scope, or (only
  inside `ensures`) `result`.
- Unary `!` (bool) and `-` (signed numeric only).
- Binary `+ - *`, `== != < <= > >=`, `&& ||`.
- `if`/`else` where both branches share one sort.
- A `Block` whose statements are all immutable `let` (no `let mut`) and
  whose tail is itself supported.

Rejected, each with one closed [`subset::UnsupportedReason`] variant rather
than a generic "translation failed": closures, any kind of call (function,
method, `super`), floats, `char`/string/bytes/array literals, records,
variants, `match`, `try`, field projection, mutable locals, `assign`,
`unsafe`, `while`, `for`, and (see below) `/` and `%`.

### Explicitly deferred, not merely unimplemented

- **Division and remainder.** SEMAPRAX's `/`/`%` truncate toward zero
  (matching Rust and C99), while SMT-LIB2's built-in `div`/`mod` are
  Euclidean (always non-negative remainder). The correct encoding needs a
  sign-correction term — for `b != 0`, with `ediv`/`emod` the built-in
  Euclidean quotient/remainder: `q_trunc = ediv + sign(b)` and
  `r_trunc = emod - |b|` whenever `a < 0 && emod != 0`, otherwise
  `q_trunc = ediv`, `r_trunc = emod`. That correction is algebraically
  straightforward but has not been given its own hostile-input test suite
  here (extrema, both signs of both operands, `MIN / -1`), and shipping an
  unverified arithmetic encoding directly contradicts the honesty bar this
  tranche is held to. Division/remainder are a closed, one-line follow-up:
  add the correction to `translate::translate_binary`, add the sign-and-zero
  divisor obligations, and add the property tests before enabling it.
- **Pure-call inlining or summaries, loops, recursion, effects, floating
  point, heap aliasing, records/variants/match.** Named out of scope by the
  issue itself; nothing here should be read as a smaller step toward them
  without its own design.
- **`cvc5`.** Not installed on any host this tranche was developed or
  evidenced on. [`solver::Provisioning::identity`] and
  [`solver::ENV_Z3_PATH`] are Z3-specific; adding a second transport is a
  new `Provisioning` variant plus its own SMT-LIB2 output dialect handling,
  not a rename.

## Numeric encoding

SEMAPRAX integers are **checked**, not wrapping: `i64: 42` in the agent
quick reference is documented as "default integer, checked overflow", and
an out-of-range operation traps rather than silently producing a wrapped
value. This matters directly for one of the issue's named failure modes:
*"Using mathematical integers where runtime operations wrap or check can
prove the wrong property."* For a **wrapping** language, mathematical
(`QF_LIA`) integers would indeed prove the wrong property — they would
accept an addition that actually wraps around as if it produced the
unbounded sum. For SEMAPRAX's checked semantics, mathematical integers are
instead exactly right: when a checked operation does not trap, its result
*is* the exact mathematical value, by definition. The soundness burden is
therefore not "model wraparound" but "prove every operation's checked value
actually stays in range wherever it is reachable" — which is what the
side-obligation machinery below does.

Each admitted type gets one `Int`-sorted SMT-LIB2 constant per free
variable (parameter, `let`-bound skolem constant, or `result`), plus one
unconditional range axiom `(and (>= x MIN) (<= x MAX))` for its exact
representable range (`i64`: `[-2^63, 2^63-1]`; `i32`: `[-2^31, 2^31-1]`;
`u8`: `[0, 255]`; `usize`: `[0, 2^64-1]`, since SEMAPRAX's `usize` is a
target-independent checked unsigned 64-bit semantic integer, not a host
pointer width). Negative numerals render as `(- <magnitude>)`, since
SMT-LIB2 numeral tokens are non-negative only.

Every `+`/`-`/`*`/unary `-` contributes one **side obligation**: the
mathematical result of that specific operation must stay within its type's
range, guarded by the exact path condition under which the real runtime
would evaluate it (see "Path-sensitive obligations" below). This is not an
approximation of checked overflow — for `+ - *` and unary `-` over
bounded-width integers, "the mathematical result equals the checked result"
is exactly the definition of "did not trap."

### Path-sensitive obligations

An operation inside an `if` branch, or inside the right operand of `&&`/
`||`, only actually executes under the real runtime when its guard holds
(the branch was taken; the left operand's truth value did not already
short-circuit). `translate::Ctx` threads an accumulated guard term through
every recursive call and attaches it to each side obligation as
`(=> guard formula)`, so an operation that can only overflow on an
unreachable branch is correctly never asserted as a hard failure. `requires`
clauses and the function body itself use a constant `"true"` guard, which is
sound specifically because the whole query already asserts every `requires`
term as an unconditional fact in the same solver context — see
`translate::translate_function`'s doc comment for why that makes clause-order
short-circuiting a non-issue for `requires` (unlike for `&&`/`||` inside one
expression, which still needs path-sensitive guarding).

## The query

For `ensures` clause `i`, `render_postcondition_script` builds one
`(check-sat)` query proving

```
requires_conjunction  =>  (well_definedness_i  AND  ensures_i)
```

by asserting `requires_conjunction` and `(not (and well_definedness_i
ensures_i))`, where `well_definedness_i` is the conjunction of every
`(=> guard formula)` side obligation from parameters/`requires`/the body
(shared across every clause) plus this clause's own. `unsat` means the
implication holds for every input satisfying the range axioms and
`requires` — a genuine proof, not merely "the ensures clause looks true."
Bundling well-definedness into the same query is deliberate: a checked-
arithmetic trap prevents `ensures` from ever being evaluated at runtime, so
"the postcondition holds" and "the postcondition holds and nothing traps
first" are the same real-world property for this subset.

`render_precondition_consistency_script` instead just asks whether
`requires_conjunction` is satisfiable at all. `unsat` here is not a proof
that any obligation holds — it means the precondition can never be
satisfied, so the function's body can never run under any input. This is
reported as `DischargeOutcome::Inconclusive` with an explicit "contradictory
precondition" reason, never folded into `Proved`, since "vacuously
unreachable" and "this obligation was proved" are different findings a
reader must not be able to confuse.

## Provisioning and process bounds

[`solver::provision_from_env`] reads exactly one environment variable,
[`solver::ENV_Z3_PATH`] (`SEMAPRAX_SMT_Z3_PATH`), and only accepts an
absolute path to an existing file — never a bare name resolved against
`PATH`, never a compiled-in default location. An unset, empty, relative, or
missing path is the ordinary "not provisioned" case
(`Verdict::NotProvisioned` / `DischargeOutcome::Inconclusive`), not an
error: this module must add zero ambient process authority to a build that
never opted in.

[`solver::run`] spawns the provisioned binary with a single `-in` argument
(SMT-LIB2 script over stdin), writes the script from a dedicated thread (to
avoid the classic pipe deadlock), and reads stdout/stderr through two
capped reader threads that stop consuming at `RunLimits::max_output_bytes +
1` bytes. A wall-clock poll loop enforces `RunLimits::timeout` independently
of the script's own embedded `(set-option :timeout ...)`, killing the
process if it fires. Capacity is checked before timeout in the result
mapping, since a solver that floods its output pipe past the cap stalls on
a full pipe once the capped reader stops draining it — the wall clock then
also fires, but the capacity bound is the real cause and must be reported
as such.

**Known limitation:** memory and process-tree limits (e.g. `ulimit`/cgroup
enforcement, killing a solver's own forked children) are not implemented —
adding a new Cargo dependency was out of scope for this tranche (shelling
out to a provisioned binary over SMT-LIB2 text was the explicit
alternative), and there is no dependency-free, portable way to set a memory
rlimit or reap an entire process group from `std` alone. Wall-clock time and
output-byte bounds are enforced; a solver process that allocates unbounded
memory without producing unbounded output, or that forks helper processes
of its own, is not bounded by this tranche. [`solver::solver_version`]'s
`--version` probe is likewise unbounded by time (it is a short, trusted,
already-provisioned auxiliary call, not the main discharge path).

`Verdict` distinguishes `Unsat`, `Sat`, `Unknown`, `Timeout`,
`CapacityExceeded`, `Crash`, `Malformed`, and `NotProvisioned` as seven
disjoint outcomes; nothing here ever collapses `Unknown`/`Timeout`/
`Crash`/`Malformed`/`NotProvisioned` into either `Unsat` or a validated
`Sat`.

## Model parsing and validation

A `sat` verdict's model is untrusted external tool output. `model::
parse_model` only recognizes `(define-fun <name> () <Int|Bool> <literal>)`
entries with a nullary parameter list and a literal (or `(- <literal>)`)
body; anything else — an uninterpreted function, a non-nullary entry, a
compound value expression, trailing tokens, an unterminated list, a
duplicate name — is a parse failure, reported as `Inconclusive`, never a
best-effort partial read.

A name the model omits (the issue's *"models may omit unconstrained
values"*) defaults to `0`/`false` in `replay::model_value_to_runtime`: since
the solver only omits a name when the falsified formula does not depend on
it, any representable value is an equally valid witness.

`replay::replay_function` then re-evaluates `requires`, the body, and
`ensures` directly against `crate::ast`, using its own independent
checked-arithmetic implementation (not sharing code with
`translate.rs`, so a shared bug in both halves cannot silently agree). Three
outcomes:

- `Trapped`: a checked operation actually overflows for these concrete
  values — a genuine counterexample.
- `EnsuresViolated`: the body returns normally but the named `ensures`
  clause evaluates to `false` — a genuine counterexample.
- `Inconsistent`: `requires` is false under replay (meaning the SMT
  encoding produced a model that contradicts a fact the query asserted —
  a translation bug, not a real input), or neither a trap nor a violation
  occurred (the `sat` verdict is spurious relative to this evaluator). This
  is reported as `Inconclusive`, never as a proof of anything.

`DischargeOutcome::Refuted` is constructed **only** from a validated
`Trapped`/`EnsuresViolated` replay outcome — never from an unparsed or
unvalidated model.

## Refutation has no assurance class

The Assurance Manifest v1 lattice (`AssuranceClass`) has nine classes:
`open`, `assumed`, `attempt_inconclusive`, `test_evidenced`,
`runtime_guarded`, `compiler_proved`, `model_checked`, `smt_proved`,
`theorem_proved`. None of them means "definitively refuted" — every class
in that closed vocabulary is a flavor of positive evidence. `to_method_
record` therefore returns `None` for `DischargeOutcome::Refuted`: no method
record is ever produced from a validated counterexample. Reporting it as
`attempt_inconclusive` would misrepresent a definitive finding as a mere
non-attempt, which is strictly more dangerous than reporting nothing —
a caller might treat "inconclusive" as "try again later" rather than "this
program has a real, concrete, reproduced defect." Turning a `Refuted`
outcome into a hard compile diagnostic (or a new lattice class) is future
work belonging to the lattice's and the diagnostic pipeline's own owners,
outside the file lease this tranche was built under; `DischargeOutcome::
Refuted`'s `replay`/`script_digest` fields already carry everything such a
caller would need.

## Cache key

`cache::cache_key` hashes, length-prefixed (preventing the same boundary-
aliasing class of bug `obligation_id` already guards against): the exact
obligation locator, the exact rendered SMT-LIB2 script text (already a
complete function of the source's parameter types, contract clauses, and
body — a source edit that changes meaning always changes this string),
solver identity, solver version, timeout, the full sorted assumption-id
list, and a target/policy string. `tests.rs`'s
`hostile_cache_substitution_a_stale_proof_cannot_survive_any_single_field_
change` exercises the property the issue names directly: changing any one
of those fields must turn a cache hit into a miss.

This tranche ships the key computation and an in-memory, process-local
`DischargeCache`, not a persisted cross-run store: a durable cache is its
own artifact with retention, eviction, and multi-process-safety concerns
that deserve their own hostile-input review, deliberately deferred rather
than shipped unreviewed.

## Integration status

This is the section to read before assuming any of the above is wired into
`generate()`. It is not, and the reason is a real, load-bearing gap this
tranche's audit found rather than papered over:

1. **The manifest's `nonclaims` array is unconditional.**
   `assurance_manifest::render::NONCLAIMS_JSON` always includes
   `"no_smt_solver_invoked"` regardless of what `ExternalRecords` supplies.
   Merging a genuine `smt_proved` method record (produced by an actual
   solver run) into a manifest today would render an envelope that
   simultaneously claims "no SMT solver was invoked" and carries a method
   record whose `tool` is `"z3"` — a direct self-contradiction. Fixing this
   means making that nonclaim conditional on whether any external record's
   `tool`/`class` indicates a real solver ran, which is `render.rs`'s
   change to make, not this tranche's: `render.rs` is not part of this
   tranche's file lease, and it is under active concurrent development
   alongside this issue.
2. **`ExternalRecords` cannot yet merge a second method into an obligation
   `derive_obligations` already populated.** `validate_obligations_and_
   assumptions` in `assurance_manifest.rs` rejects any external obligation
   whose id collides with an automatically derived one (`SPX-Z101`,
   *"an external record collided with an automatically derived obligation"*)
   — which is exactly what happens for every `precondition`/`postcondition`
   obligation, since `derive.rs` already emits one with a `runtime_guarded`
   method for every `requires`/`ensures` clause. `tests.rs`'s
   `merging_an_smt_method_into_an_already_derived_obligation_fails_closed_
   today` locks in this exact, correct (fail-closed, not silently dropped)
   behavior as a regression test, not a workaround. Supporting genuine
   multi-producer method merging into one obligation id is the "candidate-
   assurance join" #129 was scoped to build; extending it to accept a
   second method for an obligation `derive.rs` already populated belongs to
   that module's owner.

Until both land, this tranche's real, demonstrated capability is: given one
supported function, produce a script-rendering, solver-backed, replay-
validated `DischargeOutcome` and its corresponding `MethodRecord` — fully
tested, including against a real provisioned Z3 — but not yet a way to
attach that record to the manifest a plain `generate()` call already
produces for the same obligation. `postcondition_obligation_id` computing
byte-identical ids to `derive.rs`'s own locator convention is exactly what
makes that future join possible without inventing a second id scheme.

## Runtime-guard fallback

Every `MethodRecord` this module produces (`Proved` and `Inconclusive`
alike) sets `runtime_fallback: true`: an SMT proof is additional assurance
alongside the compiled trapping guard `derive.rs` already attaches, never a
replacement for it, matching the issue's explicit *"Removing runtime guards
on `unknown` or timeout"* prohibition and the repository's own invariant
that *"a settlement or concurrency model is proof data, not permission to
perform a physical finalizer, spawn runtime work, or publish an artifact."*
No code path in this tranche ever removes, disables, or conditions a
compiled guard on any discharge outcome; deciding when an explicit
target/profile policy is allowed to do so is exactly the policy machinery
named in the issue's step 7 and is out of this tranche's scope.

## Evidence

Local, fixture-backed unit and integration tests exercise script
rendering, path-sensitive obligation guarding, model parsing (including
hostile malformed/duplicate/non-nullary cases), replay validation
(including the "model doesn't actually reproduce a failure" case), the
cache key's hostile-substitution property, and the `SPX-Z101` collision
documented above — all without a solver process. A second, `#[ignore]`d
tranche of tests requires `SEMAPRAX_SMT_Z3_PATH` and was run once locally
against a provisioned Z3 to confirm: a true postcondition proves; a false
one refutes with a validated counterexample; the exact `i64::MAX` extremum
refutes via a validated trap; a contradictory precondition is reported as
such; every admitted numeric type (including `bool`) proves a trivial
identity postcondition; and a branch-sensitive `if` postcondition proves.
This is local, developer-machine evidence, not a hosted or CI-provisioned
run — see the top-level report for exact counts.
