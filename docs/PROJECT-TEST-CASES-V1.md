# Project Test Cases and Runtime Failure Report v1

Status: implemented for the in-process project runner (`semaprax run` and
`semaprax test` over a `semaprax.toml` project). The gate is
`tests/project.rs::developer_loop` together with the unit tests in
`src/project/execution/tests.rs`. The completion matrix owns product status.

Audience: coding agents and people running project tests, tool authors
consuming the `semaprax.project-execution.v1` envelope, and compiler
contributors.

This reference owns two additive behaviors of the reference interpreter's
project runner: named test cases inside the manifest-declared test module, and
the contract-failure detail that accompanies a language failure. It extends
the `run`/`test` contract in [Project Manifest v1](PROJECT-MANIFEST-V1.md)
without changing the envelope schema string, the normalized status object,
the entry closure, or the test module's `main`.

## Named test cases

A test module is a module listed under `tests` in `semaprax.toml`. Its `main`
is the test closure exactly as before: `semaprax test` evaluates it and it
passes only by returning `0`.

In addition, every function of that module that satisfies all of the following
is a *case* and is executed on its own:

- its display name starts with `test_` (`TEST_CASE_PREFIX`);
- it takes no parameters and returns `i64`;
- it carries an explicit `@id`.

A `test_`-prefixed function of any other shape, such as `fn test_helper(value:
i64) -> i64`, is an ordinary function and is not a case. A function of another
module is never a case, whatever its name. Cases are ordered by stable
identity, which is the order of the linked program's function index.

Each case runs after `main`, on its own fixed 64 MiB stack, with the whole
`--max-steps` budget; the envelope's top-level `fuel` still describes `main`
alone. Every case is admitted under the same interpreter profile and closure
scan as `main`; a case whose closure is outside the profile fails the command
with the same selection diagnostics `main` would produce.

The command passes when `main` and every case return `0`
(`ProjectExecution::command_succeeded`).

```semaprax
module calculator.tests;

@id("calculator.tests.main")
fn main() -> i64
{
    0
}

@id("calculator.tests.test_add")
fn test_add() -> i64
{
    if 19 + 23 == 42 { 0 } else { 1 }
}
```

## Human report

`semaprax test` prints exactly one of:

- `project tests passed` when `main` returns `0` and there are no cases;
- `project tests passed (N named cases)` when `main` and all `N` cases pass;
- otherwise, on stderr with exit status `1`, one `failed <stable-id>: <outcome>`
  line for `main` if it failed and then for each failing case, a summary
  `project tests failed: <failed> of <total> in <test-module>` where `total`
  counts `main` plus every case, and a `help:` line.

`<outcome>` is `returned <value>`, `language status <status-json>`, `step
budget exhausted`, or `call-depth bound exceeded`. When the outcome is a
contract failure with retained detail, two indented lines follow the `failed`
line (see below). The help line reads `a test passes by returning 0; a nonzero
return is the failing check's code or count` and, when the module has no
cases, continues `, so give each check its own `fn test_<name>() -> i64` in
the test module to have it reported by name`.

`semaprax run` keeps its lines `project execution failed with language status
<status-json>`, `project execution exhausted its step budget`, and `project
execution exceeded its call-depth bound`; the contract-failure lines follow the
first of them when detail is retained.

## Contract-failure detail

When evaluation ends in a violated `requires` or `ensures` clause, the
interpreter records at the failing frame:

- the stable identity of the function whose clause failed;
- the phase, `requires` or `ensures`;
- the clause text: the clause expression's span in the declaring file's
  retained source, trimmed; when that slice is unavailable, spans several
  lines, or exceeds 4096 bytes, the clause's revision-scoped expression
  identity is reported instead;
- the call's parameters in declaration order, each with its name, its
  name-independent type key (`i64`, `bool`, `u8`, ...), and its value. Scalars
  render as source literals with their width suffix (`-7`, `3i32`, `255u8`,
  `true`, `'a'`); owned or borrowed data renders as its kind and length
  (`<string 3 bytes>`), never its bytes.

The detail is data for reports. It never changes the normalized status, which
remains the `semaprax.status.v1` object every backend agrees on, nor cleanup,
result publication, or exit status.

The human form is two lines indented by two spaces:

```text
  contract: requires right != 0 in calculator.divide
  arguments: left = 1, right = 0
```

`arguments: none` names a zero-parameter function.

## Envelope additions

The `semaprax.project-execution.v1` envelope is unchanged for the entry role
except inside a `language_failure` outcome. Both additions are covered by the
payload digest and by `project::verify_execution_envelope`.

A `language_failure` outcome may carry a `failure` member after `status`:

```json
{"kind":"language_failure","status":{...},"failure":{"function":"calculator.divide","phase":"requires","clause":"right != 0","arguments":[{"name":"left","type":"i64","value":"1"},{"name":"right","type":"i64","value":"0"}]}}
```

Verification requires `failure` to have exactly those keys, a `phase` equal to
the status's contract code, every text field nonempty and at most 4096 bytes,
and each argument to have exactly `name`, `type`, and `value`.

A test-role envelope always carries `cases` after `outcome`, an array that is
empty when the module declares no case:

```json
"cases":[{"stable_id":"calculator.tests.test_add","name":"test_add","fuel":{"steps_used":13,"max_steps":1000000},"outcome":{"kind":"returned","type":"i64","value":"0"}}]
```

Each element has exactly `stable_id`, `name`, `fuel`, and `outcome`; `name`
starts with `test_`; `fuel.max_steps` equals the envelope's `limits.max_steps`
and `steps_used` does not exceed it; `outcome` uses the same closed vocabulary
as the top-level outcome, including `failure`. An entry-role envelope must not
carry `cases`; a test-role envelope must. The envelope's key order, nonclaims
list, and digest domain are unchanged, so a consumer that reads fields by name
keeps working, while one that pins whole test-envelope bytes observes the new
`cases` member.

## Nonclaims

- No filesystem discovery. The envelope's `no_test_discovery` nonclaim holds:
  cases are selected by name inside the linked program of the manifest-declared
  test module; no file, directory, or module outside `sources` and `tests` is
  consulted.
- No isolation between cases beyond a fresh evaluator and step budget; cases
  share nothing at runtime because the language has no mutable globals.
- Arithmetic failures (division by zero, overflow) carry no frame detail; only
  contract clauses do.
- The native path (`semaprax run file.spx --native`, `build --target native`)
  reports contract failures with the same repair facts as the interpreter:
  canonical clause text, persistent function identity, and parameters in
  declaration order with their observed values. Its normalized status and
  exit 70 contract are unchanged.
- The prepared daemon interpreter and its Source Trace
  ([Prepared Project Interpreter v1](PROJECT-PREPARED-INTERPRETER-V1.md))
  retain the detail internally but render neither `failure` nor `cases`; their
  wire bytes are unchanged.
- A `test_` function of a non-admitted shape is not a case. The human report
  of `semaprax test` prints one stderr line per such function, `note: `<name>`
  is not a test case: <rule>; …`, naming the first rule it misses (parameters,
  a non-`i64` result, or a missing explicit `@id`); the JSON envelope is
  unchanged and no compiler diagnostic is emitted.
