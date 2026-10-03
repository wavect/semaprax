# Testing

Tests in Semaprax are ordinary modules whose functions return `i64`:
`0` passes, anything else fails. The runner reports failures by stable id,
so a failing suite tells you exactly which check broke.

## Run a known suite first

From the repository root, run:

```sh
semaprax test examples/calculator-project/semaprax.toml
```

The runner should report that the project tests passed. Open its `src/tests.spx`
to see the checks. The example below belongs in that calculator project; it
imports a function from `calculator.core` and is not a standalone program.

## A test module

```semaprax
module calculator.tests;
use function @id("calculator.add") from calculator.core as add;

@id("calculator.tests.test_add")
fn test_add() -> i64
{
    if add(19, 23) == 42 { 0 } else { 1 }
}

@id("calculator.tests.test_add_zero")
fn test_add_zero() -> i64
{
    if add(0, 0) == 0 { 0 } else { 2 }
}

@id("calculator.tests.main")
fn main() -> i64
{
    0
}
```

Conventions that matter:

- **One `fn test_<name>() -> i64` per check**, each with its own `@id`.
  Every zero-parameter `test_*` function runs independently; a failure
  reports `failed calculator.tests.test_add: returned 2`.
- **Distinct nonzero codes per assertion** inside one test (`1`, `2`, …)
  pinpoint which assertion failed without rerunning.
- List the module in the manifest: `tests = ["calculator.tests"]`.

```sh
semaprax test semaprax.toml          # prints "project tests passed"
semaprax test semaprax.toml --json   # machine-readable cases envelope
```

## Contracts are tests that never rot

A `requires`/`ensures` pair is checked on every run, including under `test`.
When a contract fails, the report names the function, the clause, and the
argument values:

```text
contract: requires right != 0 in calculator.divide
arguments: left = 1, right = 0
```

Write the contract first. Then test valid inputs at useful edges, such as
zero, one item, and the largest value the function explicitly accepts.
For a function requiring a nonzero divisor, ordinary passing tests should
supply a nonzero divisor.

Test rejection separately. A deliberately invalid call triggers a failure;
it does not become a passing `test_*` case merely because the failure was
intentional. Keep that call in a separate fixture and have the surrounding
harness assert the failure status. This keeps the normal test suite green for
the right reason.

For expected user-facing errors, return `Result` and assert its error case
instead of intentionally breaking a precondition. See
[Contracts and effects](../language/contracts-effects.md).

## Prove that the test can detect a mistake

Temporarily change `add(19, 23) == 42` to `add(19, 23) == 41` and run the suite.
The corresponding test must fail. Restore the expected value and run it again.
This small negative control catches tests that never reach their assertion.

Give each test one clear purpose. A function named `test_add_zero` is easier
to diagnose than a long `test_everything` function with unrelated checks.

## The verification ladder

Run these in order; stop at the first failure:

```sh
semaprax fmt . --check        # canonical layout (lists manifest-order diffs)
semaprax check semaprax.toml  # types, contracts, effects, ownership
semaprax test semaprax.toml   # executable checks
semaprax build semaprax.toml --target web -o dist/web   # target acceptance
```

`fmt .` parses every file before rewriting any, and `check`/`test`/`build`
all accept a directory or manifest path in v0.7.0. For CI, add
`semaprax lock semaprax.toml --compare <base.lock>` to fail on breaking
interface changes.

Exact test-case semantics: [Project Test Cases v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-TEST-CASES-V1.md).

## Check handbook examples when editing documentation

The handbook's marked runnable modules and its multi-file tutorial can be
checked with the accompanying script, from the repository root:

```sh
python3 scripts/check-handbook.py --compiler /absolute/path/to/semaprax
```

The script formats temporary copies, checks them, runs the marked examples,
and compares their output. It also checks local links and chapter navigation.
`--structure-only` performs just the documentation checks and explicitly skips
compiler execution. Unmarked reference snippets are not runtime smoke tests.
