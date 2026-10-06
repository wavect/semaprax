# Laws and proofs

After this page you can write a `law`, which is a rule with its own stable
`@id`, select it in a Project, and list it. A contract describes one function
call. A law names a rule so a project can keep it while the code changes, for
example during a refactor or an agent-proposed edit.

## Write a law

A law lives in a law module, not in a standalone program. The committed
`examples/native-law-project` selects this one:

```semaprax
module native_law.laws;

@id("native-law.add.right-nonnegative")
law contract "native-law.add" requires (right: i64)
    right >= 0
    evidence theorem_proved;

@id("native-law.order.total")
law relational (left: i64, right: i64)
    left <= right || right < left
    evidence smt_proved;
```

| Part | Meaning |
| --- | --- |
| `law contract "id" requires (…)` | A rule that selects one `requires` clause of the function with that `@id`. |
| `law contract "id" ensures (…)` | The same for an `ensures` clause. It may bind `result`. |
| `law relational (…)` | A rule between typed values, with no function. |
| `(right: i64)` | The variables. Each has a scalar type. They are the whole scope. |
| `evidence …` | The kind of evidence the rule needs. |

The rule is a pure scalar expression: literals, the variables, and operators.
Calls, fields, records, branches, and quantifiers are refused. The evidence
kinds are `runtime_guarded`, `compiler_proved`, `model_checked`, `smt_proved`,
and `theorem_proved`.

Declaring a law records the rule and the evidence it needs. It does not prove
anything. Checking a project admits the law and leaves its row open until
evidence arrives.

## Select the law module

The Project manifest is `semaprax.manifest.v2`. List the file in both
`sources` and `law_sources`:

```toml
[modules]
entry = "native_law.app"
sources = ["src/LAWS.spx", "src/app.spx", "src/core.spx", "src/tests.spx"]
law_sources = ["src/LAWS.spx"]
tests = ["native_law.tests"]
```

`LAWS.spx` is only a naming habit. The explicit `law_sources` entry is what
selects it. See the [example manifest](https://github.com/wavect/semaprax/blob/main/examples/native-law-project/semaprax.toml).

## List the laws

From a repository checkout:

```sh
semaprax check examples/native-law-project
semaprax query examples/native-law-project --kind law --json
semaprax project-assurance-manifest examples/native-law-project/semaprax.toml
```

The query returns both law IDs above. The assurance manifest lists the
project's obligations and the evidence for each. Keep it with the source
revision it describes.

More law shapes live in `examples/law-packs/` (collection, finite retry,
architecture, foreign boundary, money state).

## Words you will see

| Term | Meaning |
| --- | --- |
| Proposition | The rule, such as `left <= right \|\| right < left`. |
| Proof obligation | A rule that needs evidence under the selected policy. |
| SMT solver | A tool such as Z3 that checks formulas in supported theories. |
| Theorem prover | A tool such as Lean used by the theorem-checking route. |
| Assumption | A condition the proof relies on. It stays visible in the result. |
| Counterexample | An input that shows a claimed rule fails. |
| LawSet | The selected inventory of laws the project must account for. |

## Check a law with an installed solver

`project-proof-check` runs one selected tool against one law or one function
postcondition. It takes an **absolute** manifest path, the tool, its executable
and exact version line, and a host profile. Start with its help:

```sh
semaprax help project-proof-check
```

Replace the angle-bracket values in this template. Do not paste it as is:

```text
semaprax project-proof-check <absolute-manifest> --tool z3 --executable <absolute-z3-path> --version-line <exact-version-line> --host-profile trusted-local --law native-law.order.total
```

`trusted-local` means you trust local tool execution. A solver somewhere on
`PATH` does not replace naming the executable and its version. Add
`--workflow summary` or `--workflow detail` for the strict-law review route.
Witness values are hidden unless you pass `--show-witness-values`.

## Keep the rule while you repair the code

When a proof fails, read the subject, the assumptions, and the reported reason.
A run-time guard, a test result, and a solver proof answer different
questions, so keep the evidence class with the verdict.

The protected-law workflow keeps an independent baseline. A repair changes the
implementation and is then checked again. Removing a law, loosening a
precondition, or changing the selected set is a specification change and needs
its own approval. Cached proof work is reused only when it matches the current
source and tool, so an old report never stands in for a new check.

**Next:** [Source map](../reference/source-map.md).
References: [Native Law Declarations v1](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-LAW-DECLARATIONS-V1.md),
[Law Set v1](https://github.com/wavect/semaprax/blob/main/docs/LAW-SET-V1.md),
[Protected Law Intent v1](https://github.com/wavect/semaprax/blob/main/docs/PROTECTED-LAW-INTENT-V1.md).
