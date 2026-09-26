# Contracts and effects

Contracts say what code **means**; effects say what code **touches**. Both
live in the signature, both are checked, and both show up in the semantic
graph — so agents and reviewers see them without reading the body.

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
