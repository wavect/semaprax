# Contracts and effects

Contracts say what code **means**; effects say what code **touches**. Both
live in the signature, both are checked, and both show up in the semantic
graph — so agents and reviewers see them without reading the body.

## Read a contract in plain language

Think of a function call as an agreement. The caller supplies inputs that meet
`requires`. The function supplies a result that meets `ensures`. `result` is a
special name for that returned value.

For example, `requires value >= 0` means “call me with a nonnegative value.”
`ensures result >= 0` means “my returned value will be nonnegative.” Start with
one useful rule and add the edge cases that matter to your application.

## Contracts: requires / ensures

```semaprax
module examples.meaning;

@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}
```

- `requires` is a precondition on the arguments. `ensures` is a postcondition;
  `result` names the returned value.
- Clauses sit between the signature and the body and belong to the interface —
  changing them changes the declared meaning.
- A safe profile turns obligations it can't prove statically into runtime
  guards, and test failures report the clause, the function, and the argument
  values (`contract: requires right != 0 in calculator.divide`).

**Best practice:** write contracts for every non-trivial function. They are
executable documentation: a precondition captures what you assumed, a
postcondition captures what you promised, and both get re-checked on every
`test` run. Start with bounds (`value >= 0`), exact results for pure helpers
(`result == left + right`), and non-emptiness for builders.

## Decide between a contract and Result

Use a precondition for something the caller must establish before calling.
Use `Result<T, E>` when an expected outcome, such as invalid user input, should
be handled by ordinary application code. A deliberate contract violation in
a normal test makes that test fail; see [Testing](../practices/testing.md) for
how to keep rejection checks separate from the passing suite.

For named project-wide rules and selected solver-backed checks, continue with
[Laws and proofs](laws.md). It explains how a law identifies the rule you want
to keep while the implementation changes.

## Effects: permit / uses

No function touches the outside world silently. The module **permits** effects
for the file; every function that performs an effect — or calls one that
does — **declares** it:

```semaprax
module app.ticking;

permit { clock.read }

@id("flow.tick")
fn tick(value: i64) -> i64
    uses { clock.read }
{
    value + 1
}

@id("app.main")
fn main() -> i64
    uses { clock.read }
{
    tick(41)
}
```

In this example, `tick` only adds one. The declared `clock.read` effect shows
how effect requirements propagate through a call; it does not itself read a
clock. The [first program](../getting-started/first-program.md) uses a real
`stdout_write` operation to demonstrate an observable effect.

Missing `permit` is `SPX-E101`; missing `uses` is `SPX-E102`. The effect list
is closed and explicit:

| Effect | Operations | Notes |
| --- | --- | --- |
| `process.stdout.write` | `stdout_write` | Returns bytes written; single-file `run` uses a bounded transcript |
| `clock.read` | time reads | Declare transitively through callers |
| `network.connect`, `network.read`, `network.write` | `net_connect`, `net_send`, `net_recv`, … | TCP client ops via an injected provider; `net_recv` not admitted in `while` bodies |
| `fs.read`, `fs.write` | `file_read`, `file_write_new`, stat/list/… | Bounded relative paths; writes create new files, never overwrite |

Compiler and generated code gain **no** ambient authority from being
installed: no filesystem, process, network, or signing access unless a
declared effect and an explicit provider grant it.

## Keep declaration and execution separate

Three things work together: the module permits an effect, the function declares
it, and the selected host provides the operation. A **host** is the environment
that runs the program and supplies external services. Declaring `network.read`
does not create a socket or choose credentials.

This separation is especially useful in tests. A test host can supply fixed
responses, while an explicitly configured runtime host performs the real I/O.
See [Profiles](../projects/profiles.md) before moving an effectful helper to a
new execution target.

## Querying meaning

Because contracts and effects are structured data, you can ask for them
without reading source:

```sh
semaprax context examples/meaning.spx math.add --depth 1 --filters contracts
semaprax doc examples/meaning.spx
```

`context` returns one declaration's neighborhood as bounded JSON; `doc`
renders declarations, signatures, contracts, and effects as documentation.
See [Agents](../practices/agents.md) for the full query workflow.

Exact rules: [RFC 0001](https://github.com/wavect/semaprax/blob/main/docs/RFC-0001.md)
(contracts and verification) and the effect specifications under
[`docs/`](https://github.com/wavect/semaprax/tree/main/docs).
