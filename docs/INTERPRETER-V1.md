# Reference Interpreter v1

`semaprax interpret <file.spx> --function <name|stable-id>
[--arg <i64|bool literal>]... [--max-bytes N]` is a deterministic, read-only
reference evaluator that runs ONE explicitly selected explicit-ID monomorphic
effect-free scalar function directly from the resolved HIR of one verified
single-file SEMAPRAX module — no backend toolchain, no code generation, no
compilation, and no target execution. It is the first executable slice of the
completion-matrix row "Fast development lane" under Compiler and output
targets. It contains no JIT, AOT, or Cranelift machinery, no incremental
build persistence, no hot reload, no debugger mapping, executes nothing on a
target, and changes no source.

## Command

```sh
semaprax interpret <file> --function name|stable-id [--arg literal]... [--max-bytes N]
```

- `--function` (exactly once) selects one function by display name or
  persistent stable ID; an unknown token fails closed (`SPX-F102`).
- `--arg` repeats once per parameter in declaration order. A literal is
  either `true`/`false` or a canonical optionally negative decimal integer;
  non-canonical or out-of-range literals fail closed (`SPX-F103`).
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-F104`; output is
  never truncated.

## Admission model

The selected function — and every callee transitively reachable from it —
must have an explicit stable identity, be monomorphic, declare no effects,
take only by-value direct `i64`/`bool` parameters, and return direct
`i64`/`bool`. Anything else fails the whole command closed (`SPX-F102`)
with exactly one closed reason: `automatic_identity`,
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, `unsupported_result_type`, `generic_call`,
`import_call`, `record_construction`, `variant_construction`,
`record_update`, `record_projection`, `match_expression`,
`try_expression`, `place_projection`, `unsupported_callee`, or
`unsupported_scalar_operation` (the backend-unlowerable shapes:
`f32`/`f64`/`u8` remainder and `char` arithmetic).

Inside that profile the interpreter evaluates the full admitted scalar
surface: `let` (including Explicit Mutation v1 `let mut`) and assignment
statements, blocks with proper scoping, `if`, lazy `&&`/`||`, unary negation
and logical not, every admitted binary operator with strict left-to-right
evaluation order and sticky first-failure selection, `i64`/`i32`/`u8`/
`char`/`f32`/`f64`/`bool` literals, requires/ensures contracts (evaluated in
order at entry and after body success respectively, with `result` bound for
ensures), checked arithmetic over `i64`/`i32`/`u8` reusing the compiler's
exact `runtime_status` normalization table, total IEEE-754 arithmetic for
floats, and calls to other admitted functions (including recursion).

## Outcome envelope

`interpreter::interpret` returns one canonical compact JSON envelope plus a
returned/not-returned flag. The CLI prints the envelope and exits `0` when
the outcome is a returned value, `1` when the outcome is a normalized failure
status or a capacity kind (the command did not return), and `2` for usage
errors. Diagnostics during generation also exit `1`.

Payload members in fixed order: `schema`, `source` (`path`, graph
`revision`, domain-separated source digest), `function` (`stable_id`,
`name`), `arguments` echo (`index`, `name`, `type`, canonical `value`
string), `limits` (`max_bytes`, `max_steps`), `fuel` (`steps_used`,
`budget`, `exhausted`), `outcome`, and fixed `nonclaims`. The outer wrapper
is `{"schema","digest","bytes","payload"}` where `digest` is the
domain-separated SHA-256 of the exact payload bytes
(`semaprax.interpret.payload.v1`) and `bytes` is their length.

Outcomes:

- `{"kind":"returned","type":"i64"|"bool","value":"<decimal>|true|false"}`
  for successful evaluation;
- `{"kind":"failed","status":{...}}` carrying the exact compiler-owned
  normalized status (`semaprax.status.v1`) selected by checked arithmetic or
  a false contract clause — the same statuses the native C11 and Core-Wasm
  backends report;
- `{"kind":"fuel_exhausted"}` when the step budget is consumed before the
  evaluation finishes; each expression node, statement, and contract clause
  consumes exactly one step (library default 1,000,000), exhaustion pins
  `steps_used == budget`, and exhausted evaluations are fail-closed capacity
  facts, never language statuses;
- `{"kind":"call_depth_exceeded"}` at the fixed call-depth ceiling of 256
  SPX frames.

Evaluation runs on a dedicated fixed-size-stack thread so the depth ceiling
is reachable without native stack exhaustion; this changes nothing about the
output bytes.

## Replay verification

`interpreter::verify_envelope` independently recomputes the outer payload
digest over the exact serialized payload bytes, re-checks the declared byte
count, and replays every closed derivation inside the payload: exact member
sets for every object, argument/return value grammars per declared type,
fuel-budget bounds and the invariants `steps_used <= budget` and
"`exhausted` implies `steps_used == budget`", `fuel.budget ==
limits.max_steps`, the closed outcome-kind vocabulary, and exact
reconstruction of failed statuses from the closed compiler-owned v1 tables.
Echo-only fields (paths, display names, step counts) are authenticated by
the digest but deliberately not independently re-derivable.
`verify_envelope_against_source` additionally binds the current source bytes
to the embedded source digest, failing closed (`SPX-F106`) on drift. Any
mutation anywhere in the envelope invalidates verification.

All diagnostics use the previously unused `SPX-F1xx` family:
`SPX-F101` options, `SPX-F102` selection/admission, `SPX-F103` arguments,
`SPX-F104` budget exhaustion, `SPX-F105` fail-closed evaluation guards,
`SPX-F106` envelope consistency or replay failure.

## Evidence

Executable evidence lives in `tests/interpreter_v1.rs` plus module tests in
`src/interpreter.rs`: a 28-row backend-parity corpus proving the interpreter,
native C11 at `-O0`/`-O2`, and Node/Wasm produce byte-identical result/status
transcripts (full scalar surface versus native; the whole-program
web-profile subset versus all three producers), pinned golden envelope and
fuel-exhausted envelope digests over `examples/meaning.spx`, determinism,
fuel-exhaustion accounting, the call-depth ceiling, every admission reason,
argument diagnostics, per-field tamper rejection including re-signed
forgeries, drift binding, and CLI exit-code contracts. Toolchain-dependent
parity legs skip when clang or Node is unavailable unless
`SEMAPRAX_REQUIRE_INTERPRETER_BACKEND_PARITY` is set.

Nonclaims: no JIT/AOT/Cranelift or any machine-code emission, no incremental
persistence, no hot reload, no debugger mapping, no target execution, and
read-only evaluation only. See also
[EXPLICIT-MUTATION-V1.md](EXPLICIT-MUTATION-V1.md) for the mutation forms the
interpreter shares and [PROPERTY-TESTS-V1.md](PROPERTY-TESTS-V1.md) for the
AST-level analysis sibling.
