# Contributing

SEMAPRAX values coherent semantics over feature count. Before adding syntax or
a generated surface, explain which semantic operation, verification rule, or
systems use case it enables.

## Before you change code

Read the [development guide](docs/DEVELOPMENT.md) and [agent operating
invariants](AGENTS.md). Then read the versioned specification that owns the
area you are changing. The [architecture](docs/ARCHITECTURE.md) is the single
module map; the [completion matrix](docs/COMPLETION-MATRIX.md) is the status
authority.

If this is your first change here, [First
contribution](docs/FIRST-CONTRIBUTION.md) walks one end to end: how to pick a
change the quality router keeps narrow, how to find the specification that owns
the area and the completion-matrix rows it touches, where a test of that shape
belongs among the `tests/` harnesses, which profile to run and what each one
compiles, the local hazards a newcomer otherwise hits, and which owner to update
at the end. It adds no rule; it sequences the ones this file and the development
guide already state.

Design changes affecting syntax, graph schemas, transactions, effects,
ownership, contracts, components, or ABIs should begin as an RFC or an explicit
revision to the owning RFC.

## Tests and evidence

Compiler changes normally need:

- an admitted success case;
- a stable diagnostic regression;
- canonical source round-trip coverage;
- semantic graph assertions;
- native/Wasm equivalence when runtime meaning changes;
- independent replay and hostile-input evidence when an evidence or
  transaction boundary changes.

Run the full Unix gate with:

```sh
scripts/quality.sh full
```

For a faster local loop, preview the repository-aware route and then run it:

```sh
scripts/quality.sh changed --plan
scripts/quality.sh changed
```

On hosts without a POSIX shell, reproduce the manual baseline in
[Quality gates](docs/QUALITY-GATES.md) and run the focused evidence named by
the owning specification.

## Documentation

Put user workflows and concepts in public documentation. Put exact wire and
admission rules in versioned references. Put module ownership and trust
boundaries in architecture, status in the completion matrix, sequencing in the
roadmap, and history in the changelog. Link to the owner instead of copying its
full explanation.

Do not present local, private, proof-only, simulator, diagnostic, or prior-head
evidence as a broader public or hosted claim.

## Community

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
