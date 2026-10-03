# Laws and proofs

A contract describes a function call. A **law** gives a rule its own stable
identity so a project can track the rule as implementations change. This is
useful when a rule must survive a refactor or an agent-proposed edit.

Start by reading a law, then inspect the complete example project. Add solver
configuration only after that workflow is familiar.

## Read two kinds of law

The committed native-law project contains this selected law module:

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

This is a law module selected by a Project, not a standalone executable with
`main`.

The first declaration selects a particular precondition on the function with
ID `native-law.add`. Its rule is “the right argument is nonnegative.” The law
has its own ID, separate from the function's ID.

The second is a **relational law**: it states a relationship between typed
values. It says that either `left` is at most `right`, or `right` is less than
`left`. Its variables are declared in the parentheses.

`evidence theorem_proved` and `evidence smt_proved` name the evidence required
for the law. The declaration records the requested rule and evidence class;
the proof workflow supplies and checks the evidence.

## Select the law file in the manifest

The complete example uses `semaprax.manifest.v2`. Its `sources` inventory
includes `src/LAWS.spx`, and `law_sources` selects that path as a law module.
The filename is a convention; the explicit selection is what matters.

Open the [example manifest](https://github.com/wavect/semaprax/blob/main/examples/native-law-project/semaprax.toml)
when adding your own file. Keep executable modules and law modules in their
respective roles.

## Inspect the example

Run these commands from the repository root:

```sh
semaprax check examples/native-law-project
semaprax query examples/native-law-project --kind law --json
semaprax project-assurance-manifest examples/native-law-project/semaprax.toml
```

The query should include both law IDs shown above. The assurance output lets
you inspect the project's obligations and available evidence. Keep the source
revision with the report so you know which program it describes.

## Understand the proof vocabulary

| Term | Meaning here |
| --- | --- |
| Proposition | The rule being checked, such as `left <= right || right < left`. |
| Proof obligation | A rule that needs evidence under the selected policy. |
| SMT solver | A tool, such as Z3, that checks formulas in supported theories. |
| Theorem prover | A tool, such as Lean, used by the supported theorem-checking route. |
| Assumption | A condition that the proof relies on. Keep it visible in the result. |
| Counterexample | A checked input or model showing why a claimed rule fails. |
| LawSet | The selected inventory of laws that the project must account for. |

## Run a selected installed-tool check

The CLI has a `project-proof-check` command. It takes an **absolute** manifest
path, the selected tool and executable, its exact version line, a host profile,
and either a law ID or a precise function postcondition selector.

Start with the installed help:

```sh
semaprax help project-proof-check
```

The following is a command **template**. Replace the angle-bracket values;
do not paste them literally:

```text
semaprax project-proof-check <absolute-manifest> --tool z3 --executable <absolute-z3-path> --version-line <exact-version-line> --host-profile trusted-local --law native-law.order.total
```

`trusted-local` means you explicitly trust that local tool execution. Use the
host policy appropriate to your environment. A solver installed somewhere on
`PATH` is not a substitute for the required executable and identity selection.

The workflow form adds `--workflow summary` or `--workflow detail` for the
selected strict-law review route. Witness values are redacted by default;
`--show-witness-values` makes that disclosure explicit.

## Keep the rule while repairing the code

When a proof fails, inspect the selected subject, assumptions, and reported
reason. A runtime guard, a test result, and a solver proof answer different
questions; keep the evidence class with the verdict.

The protected-law workflow keeps an independently selected baseline. An agent
repair should change the permitted implementation body, then check the new
revision. Removing the law, loosening a precondition, or changing the selected
inventory is a specification change and follows its own approval path.

Installed proof work can be reused under the supported source- and tool-bound
cache rules. Reuse still has to match the current subject; a report from an
older implementation is not a shortcut around that matching step.

**Next:** [Inspect the implementation behind the proof routes](../reference/source-map.md).
References: [Native Law Declarations v1](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-LAW-DECLARATIONS-V1.md),
[Law Set v1](https://github.com/wavect/semaprax/blob/main/docs/LAW-SET-V1.md),
and [native proof binding](https://github.com/wavect/semaprax/blob/main/src/assurance_manifest/law_set/native_proof.rs).
