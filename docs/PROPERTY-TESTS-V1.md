# Property-Test Generation v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, plus compiler contributors.

`semaprax properties <file.spx>` is a deterministic, read-only analysis that
generates bounded test inputs from admitted function signatures and evaluates
the authored `requires`/`ensures` contracts against them. It is the first
executable tranche of the roadmap item "Property tests generated from types
and contracts". It runs no target, executes no generated backend, and changes
no source.

## Command

```sh
semaprax properties <file> [--max-cases N] [--max-functions N] [--max-bytes N] [--seed N]
```

- `--max-cases` (default 64, maximum 4096): candidate input tuples generated
  per analyzed function.
- `--max-functions` (default 64, maximum 1024): functions processed in
  authored order before `function_budget` truncation.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits):
  whole-report output budget; overflow selects the longest fitting prefix and
  reports `byte_budget`.
- `--seed` (default 11400714819323198485 = `0x9e3779b97f4a7c15`): decimal seed
  for the deterministic per-parameter sampling streams.

## Admission and evaluation model

A function is admitted only when it is monomorphic, declares no effects, has
only by-value direct parameters over the admitted Copy-scalar types
(`i64`, `i32`, `u8`, `char`, `f32`, `f64`, `bool`), and returns one of those
types. Every other function is reported as `deferred` with one closed reason:
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, `unsupported_result_type`, or the first
unsupported construct found by the pre-case scan (`record_construction`,
`variant_construction`,
`record_update`,
`record_projection`, `match_expression`, `try_expression`, `generic_call`,
`unresolved_call`, `unresolved_variable`, `unsupported_callee`,
`ill_typed_expression`) or `evaluation_step_budget_exhausted`.

For each admitted function the generator produces deterministic candidates:
the first cases use a fixed boundary lattice per parameter (`0`, `±1`, `±2`,
`±3`, `i64::MIN`/`MAX`/`MIN+1`/`MAX-1` for `i64`; the same shape for `i32`;
`0`, `1`, `2`, `3`, `255`, `254`, `253` for `u8`; printable ASCII anchors, the
named escapes, and `\u{10ffff}` for `char`; finite literals
`0.0`, `±1.0`, `±2.0`, `±3.0`, `f32::MIN`, `f32::MAX`, `±0.5` for `f32` and
the analogous `f64` set; `true`, `false` for `bool`),
and later cases draw full-range samples from independent xorshift64* streams
seeded through splitmix64 mixing of `(base seed, authored function index,
parameter position)`. Sampled floats are constructed from exact 24-bit
(`f32`) and 53-bit (`f64`) magnitudes scaled by a power of two, so every
generated float is finite by construction.

Each candidate tuple is classified exactly once:

1. `requires` clauses are evaluated left-to-right under the parameter
   bindings. The first `false` clause *filters* the case — the input lies
   outside the declared domain and is not a counterexample.
2. The body is evaluated with checked arithmetic over all admitted integer
   widths, IEEE-754 float arithmetic, short-circuit booleans, lexical `let`
   (including `let mut`) bindings, plain assignment statements, bounded
   `while` loops, `if/else`, and interprocedural calls to other admitted
   local functions. Callee preconditions are re-checked at each call;
   violations surface as the `callee_requires_violated` runtime reason. A
   callee's postconditions are not evaluated. Runtime reasons are closed:
   `arithmetic_overflow`, `division_by_zero`, `remainder_by_zero`,
   `negation_overflow`, `call_depth_exceeded`, `callee_requires_violated`.
   Width-specific overflow statuses collapse into `arithmetic_overflow`.
   Floats never select a runtime failure: division by zero yields an
   infinity, exactly like the interpreter engine. Every loop iteration
   charges steps, so a non-terminating loop fails closed through the shared
   step-budget path instead of hanging; field-assignment targets stay closed
   with the aggregate reasons above.
3. With the result bound, every `ensures` clause is evaluated. The first
   `false` clause is recorded as the counterexample (clause index, canonical
   clause text, full argument tuple, observed result) and stops further cases
   for that function. All-passing tuples count as discharged.

Evaluation shares one invocation-wide step budget; exhaustion stops the run
and reports `step_budget` truncation for unprocessed functions.

## Output

The report is canonical compact JSON with schema
`semaprax.property-tests.v1` and fixed key order: `schema`, `source`
(`path`, `revision`, `sha256` over the domain-separated exact source bytes),
`seed`, `limits`, `budget` (`used_functions`, `used_cases`, `used_nodes`),
`truncation` (`truncated`, closed `reasons`: `function_budget`, `step_budget`,
`byte_budget`, plus `omitted_functions`), `summary`, `functions`, and
`nonclaims`. Analyzed entries carry `stable_id`, `name`, `outcome`,
`signature`, clause listings, per-outcome counters, sorted `runtime_reasons`,
and either `counterexample` or `null`. Integer values are serialized as
decimal strings to keep the report JSON-number safe. Widened scalar values
render canonically inside quoted JSON strings: `i64` as bare decimal,
suffixed widths with their explicit `i32`/`u8` suffixes, `char` through the
canonical escape projection (`'a'`, `'\n'`, `'\u{10ffff}'`), and floats as
their exact big-endian IEEE-754 bit pattern — the same convention the
interpreter's envelopes use — so `-0.0`, infinities, and NaN results stay
distinguishable without relying on any platform's decimal formatting.

## Widening — full Copy-scalar surface (2026-08-24)

Generation and admission widened from `i64`/`bool` to the full seven-type
Copy-scalar surface the interpreter engine already evaluates: `i64`, `i32`,
`u8`, `char`, `f32`, `f64`, `bool`. The envelope schema stays
`semaprax.property-tests.v1`; the widening is purely additive and legacy
reports over `i64`/`bool` signatures replay byte-identically. The closed
reason vocabulary shrank only by the four literal rejections that became
admissions (`float_literal`, `int32_literal`, `char_literal`,
`uint8_literal`) and by admitting bounded `while` loops; strings, records,
variants, generics, effects, method calls, match, and try remain closed with
the same reasons as before.

## Nonclaims

Property-Test Generation v1 performs no symbolic execution or SMT solving,
discharges no contract statically, minimizes no counterexamples, makes no
statistical coverage guarantee, is not a test runner, and executes no target.
Runtime failures are defined-language observations, not safety claims. The
tranche does not move any completion-matrix status.

## Evidence

```sh
cargo test --locked -p semaprax --lib properties::
cargo test --locked -p semaprax --test property_tests_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test language property_widen:: -- --test-threads=1
```
