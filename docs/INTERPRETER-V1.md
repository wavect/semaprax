# Reference Interpreter v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: language users, tool authors, and compiler contributors.

`semaprax interpret <file.spx> --function <name|stable-id>
[--arg <scalar literal>]... [--max-bytes N]` is a deterministic, read-only
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
  `true`/`false`, a canonical optionally negative decimal integer (optionally
  suffixed `i32` or `u8`, exactly the suffixes the language lexer admits),
  a floating-point literal in the language grammar (required fraction,
  optional exponent, optional `f32`/`f64` suffix, finite value only), or a
  `char` literal in the language's escape syntax (`\n`, `\r`, `\t`, `\0`,
  `\'`, `\\`, `\u{...}`). A literal binds only to the parameter type it
  canonically denotes — bare decimals are `i64`; non-canonical, out-of-range,
  or mismatched literals fail closed (`SPX-F103`).
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-F104`; output is
  never truncated.

## Admission model

The separate opt-in [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md)
adds `interpret-strings` with a distinct report identity. It does not change
this command's admission, output, or replay rules.

The selected function — and every callee transitively reachable from it —
must have an explicit stable identity, be monomorphic, declare no effects,
take only by-value direct parameters of the admitted scalar types, and return
one direct value of those same types (mixed scalar signatures are admitted).
Anything else fails the whole command closed (`SPX-F102`)
with exactly one closed reason: `automatic_identity`,
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, `unsupported_result_type`, `generic_call`,
`import_call`, `record_construction`, `variant_construction`,
`record_update`, `record_projection`, `match_expression`,
`try_expression`, `place_projection`, `unsupported_callee`,
`unsupported_scalar_operation` (the backend-unlowerable shapes:
`f32`/`f64`/`u8` remainder and `char` arithmetic), or `unsafe_boundary`
(Unsafe Boundary Mechanics v1 statements are outside the admitted surface).

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

- `{"kind":"returned","type":"<scalar>","value":"<canonical>"}` for successful
  evaluation, where the type is one of `i64`, `i32`, `u8`, `char`, `f32`,
  `f64`, `bool` and the canonical value grammar is per type: decimal for
  `i64`, the suffixed decimal (`610i32`, `253u8`) for narrower integers,
  `true`/`false` for bool, the canonical char literal for char, and the exact
  big-endian IEEE-754 bit pattern as lowercase hex (eight digits for `f32`,
  sixteen for `f64`) for floats;
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
`tests/language/interpreter_scalar_widen.rs` adds a 24-row widened-surface corpus
with the same producer contracts.

## Scalar admission widening (2026-08-23)

The admission profile is widened from direct `i64`/`bool` signatures to all
seven primitive scalar types — `i64`, `i32`, `u8`, `char`, `f32`, `f64`,
`bool` — for by-value parameters and results, including mixed signatures
(e.g. `fn f(a: i32, b: u8) -> f64`). This is an admission change only: the
engine already evaluated these types inside admitted bodies, its arithmetic,
statuses, fuel accounting, and evaluation order are unchanged, and the
envelope schema stays `semaprax.interpret.v1` with no structural member
added (the payload carries no admission-description field). Replay is
extended additively: previously accepted envelopes still verify byte-for-byte
under the same rules.

Canonical value renderings per type: integers echo as decimals, with narrower
integer widths always carrying their explicit suffix (`-7i32`, `255u8`) so
every rendering replays uniquely against one closed grammar; chars render in
the language's canonical escape syntax (`'a'`, `'\n'`, `'\u{2603}'`); floats
render as their exact big-endian IEEE-754 bit patterns (`f32` eight lowercase
hex digits, `f64` sixteen), which makes `-0.0`, infinities, and NaN payloads
directly observable without trusting any platform's decimal formatting.

Nonclaims of this widening: no strings, records, variants, generics, effects,
or Option/Result returns are admitted; `--arg` binding stays exact (a bare
decimal canonically denotes only `i64`; narrower widths require their
suffix; float literals must carry the matching precision); engine arithmetic,
status normalization, and backends are untouched; and NaN-producing
arithmetic (e.g. `0.0 / 0.0`) remains outside the cross-backend bit-exactness
guarantee because hardware default-NaN generation and Wasm/V8 canonicalization
need not agree on sign or payload — such programs are evaluated with total
IEEE-754 comparison semantics everywhere, but their NaN bits are not pinned
across producers.

Nonclaims: no JIT/AOT/Cranelift or any machine-code emission, no incremental
persistence, no hot reload, no debugger mapping, no target execution, and
read-only evaluation only. See also
[EXPLICIT-MUTATION-V1.md](EXPLICIT-MUTATION-V1.md) for the mutation forms the
interpreter shares and [PROPERTY-TESTS-V1.md](PROPERTY-TESTS-V1.md) for the
AST-level analysis sibling.
