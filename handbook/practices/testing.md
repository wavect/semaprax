# Testing

A test is a function that returns `i64`: `0` passes, anything else fails. After
this page you can write tests, read a failure, and run the full check ladder
before you push.

## Run a project's tests

Save these four files in a new `testing-demo/` directory (the `src/` files go in
`testing-demo/src/`).

<!-- handbook-project-file: {"group":"testing","path":"semaprax.toml","stdout":"42\n","test":true} -->
```toml
schema = "semaprax.manifest.v1"

[package]
name = "testing-demo"
version = "0.1.0"

[modules]
entry = "demo.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["demo.tests"]

[exports]
web = ["demo.divide"]
```

<!-- handbook-project-file: {"group":"testing","path":"src/core.spx"} -->
```semaprax
module demo.core;

@id("demo.divide")
fn divide(left: i64, right: i64) -> i64
    requires right != 0
    ensures result * right <= left
{
    left / right
}
```

<!-- handbook-project-file: {"group":"testing","path":"src/app.spx"} -->
```semaprax
module demo.app;
use function @id("demo.divide") from demo.core as divide;

@id("demo.main")
fn main() -> i64
{
    divide(84, 2)
}
```

<!-- handbook-project-file: {"group":"testing","path":"src/tests.spx"} -->
```semaprax
module demo.tests;
use function @id("demo.divide") from demo.core as divide;

@id("demo.tests.test_divide")
fn test_divide() -> i64
{
    if divide(84, 2) == 42 { 0 } else { 1 }
}

@id("demo.tests.test_divide_by_one")
fn test_divide_by_one() -> i64
{
    if divide(7, 1) == 7 { 0 } else { 2 }
}

@id("demo.tests.main")
fn main() -> i64
{
    0
}
```

```sh
semaprax test .
```

```text
project tests passed (2 named cases)
```

`test` runs the test module's `main`, then every zero-parameter `test_*`
function in that module. Each needs its own `@id`. The module must be in the
manifest's `tests` entry. A `test_*` function with another shape is not a case.

| Output | Meaning |
| --- | --- |
| `project tests passed` | `main` returned `0` and there are no named cases. |
| `project tests passed (N named cases)` | `main` and all `N` cases returned `0`. |
| `failed <id>: returned 2` | That case returned `2`. Use distinct codes per assertion. |
| `project tests failed: 1 of 2 named cases in demo.tests` | The summary line. Exit code is `1`. |

Options: `--json` for the full case list (`semaprax.project-execution.v1`),
`--max-steps N`, `--max-bytes N`. Tests run in the bounded interpreter only.
They do not exercise native or Wasm output, so build and test those targets
separately.

## Read a contract failure

A failed `requires` or `ensures` fails the case that triggered it and names the
clause and arguments. Change `divide(7, 1)` to `divide(7, 0)` and run again:

```text
failed demo.tests.test_divide_by_one: language status {"schema":"semaprax.status.v1","domain_id":"semaprax.contract.v1","code":1,"class":"contract","retryable":false}
  contract: requires right != 0 in demo.divide
  arguments: left = 7, right = 0
```

Write the contract first, then test valid inputs at the edges: zero, one item,
the largest accepted value. Do not keep a deliberately invalid call in the normal
suite: it fails the suite. Put it in a separate fixture and assert the failure
status from your harness. For expected user-facing errors, return `Result` and
test the error arm. See [Contracts and effects](../language/contracts-effects.md).

## Prove the test can fail

Change `== 42` to `== 41`, run `semaprax test .`, confirm it fails, then change
it back. A test that cannot fail checks nothing. Keep one purpose per test:
`test_divide_by_one` is easier to diagnose than `test_everything`.

## More test tools

| Tool | Use it to |
| --- | --- |
| `semaprax properties <file>` | Generate bounded inputs from signatures and contracts and evaluate them. Scalar, effect-free functions only. Options: `--max-cases`, `--max-functions`, `--max-bytes`, `--seed`. |
| `semaprax interpret <file> --function <id> --arg 1 --arg 2` | Call one function with scalar literals. |
| `semaprax assurance-policy <file> --profile <p>` | Check that each obligation is met statically, by guard or by test evidence. |
| `semaprax network-run <project> --fixture f.json` | Run a network command against a recorded fixture. |
| Law files | Prove or check declared laws. See [Laws and proofs](../language/laws.md). |

A single file given to `properties` or `interpret` needs a `fn main() -> i64`
(`SPX-T105` otherwise).

## The check ladder

Run these in order and stop at the first failure:

```sh
semaprax fmt . --check        # canonical layout, manifest order
semaprax check .              # types, contracts, effects, ownership
semaprax test .               # executable checks
semaprax build . --target web -o dist/web   # target acceptance
semaprax lock . --compare base.lock         # CI: fail on breaking interfaces
```

`fmt .` parses every file before it rewrites any. `check`, `test`, `build` and
`fmt` accept `.`, a directory, `semaprax.toml` or `--manifest-path`.

## Check handbook examples

If you edit these docs, run from the repository root:

```sh
/usr/bin/python3 scripts/check-handbook.py --compiler /absolute/path/to/semaprax
```

It formats temporary copies, checks and runs every marked example, compares
output, and verifies links and chapter navigation. `--structure-only` skips the
compiler. Unmarked snippets are not run.

References: [Project Test Cases v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-TEST-CASES-V1.md),
[Property-Test Generation v1](https://github.com/wavect/semaprax/blob/main/docs/PROPERTY-TESTS-V1.md).
