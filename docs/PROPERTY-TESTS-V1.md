# Property-Test Generation v1

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
only by-value direct `i64`/`bool` parameters, and returns direct `i64`/`bool`.
Every other function is reported as `deferred` with one closed reason:
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, `unsupported_result_type`, or the first
unsupported construct found by the pre-case scan (`float_literal`,
`int32_literal`, `char_literal`, `uint8_literal`, `record_construction`,
`variant_construction`,
`record_update`,
`record_projection`, `match_expression`, `try_expression`, `generic_call`,
`unresolved_call`, `unresolved_variable`, `unsupported_callee`,
`ill_typed_expression`) or `evaluation_step_budget_exhausted`.

For each admitted function the generator produces deterministic candidates:
the first cases use a fixed boundary lattice per parameter (`0`, `±1`, `±2`,
`±3`, `i64::MIN`/`MAX`/`MIN+1`/`MAX-1` for `i64`; `true`, `false` for `bool`),
and later cases draw full-range samples from independent xorshift64* streams
seeded through splitmix64 mixing of `(base seed, authored function index,
parameter position)`.

Each candidate tuple is classified exactly once:

1. `requires` clauses are evaluated left-to-right under the parameter
   bindings. The first `false` clause *filters* the case — the input lies
   outside the declared domain and is not a counterexample.
2. The body is evaluated with checked arithmetic, short-circuit booleans,
   lexical `let` bindings, `if/else`, and interprocedural calls to other
   admitted local functions. Callee preconditions are re-checked at each call;
   violations surface as the `callee_requires_violated` runtime reason. A
   callee's postconditions are not evaluated. Runtime reasons are closed:
   `arithmetic_overflow`, `division_by_zero`, `remainder_by_zero`,
   `negation_overflow`, `call_depth_exceeded`, `callee_requires_violated`.
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
decimal strings to keep the report JSON-number safe.

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
```
