# Testing

Tests in Semaprax are ordinary modules whose functions return `i64`:
`0` passes, anything else fails. The runner reports failures by stable id,
so a failing suite tells you exactly which check broke.

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

Best practice: **write the contract first, then a test that exercises both
sides** — one passing call and one call that would violate the precondition
(if the precondition is caller-enforced) or assert the postcondition's edge
(zero, empty, maximum). The contract guards all future callers; the test
pins today's behavior.

## The verification ladder

Run these in order; stop at the first failure:

```sh
semaprax fmt . --check        # canonical layout (lists manifest-order diffs)
semaprax check semaprax.toml  # types, contracts, effects, ownership
semaprax test semaprax.toml   # executable checks
semaprax build semaprax.toml --target web -o dist/web   # target acceptance
```

`fmt .` parses every file before rewriting any, and `check`/`test`/`build`
all accept a directory or manifest path in v0.6.0. For CI, add
`semaprax lock semaprax.toml --compare <base.lock>` to fail on breaking
interface changes.

Exact test-case semantics: [Project Test Cases v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-TEST-CASES-V1.md).
