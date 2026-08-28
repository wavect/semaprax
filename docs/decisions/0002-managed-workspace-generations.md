# ADR 0002: Managed immutable generations and an ACTIVE pivot

Audience: maintainers and compiler contributors.

- Status: accepted
- Date: 2026-08-11

## Context

RFC 0001 requires successful multi-file transactions to publish every verified
operation or none. Portable sequential replacement of ordinary source paths
cannot provide that property: after the first rename and before the last, a
reader can observe a mixed generation. The existing A0 boundary is deliberately
single-file and must remain byte-, API-, and behavior-compatible.

The repository needs one substantial but honest architecture tranche that can
publish a bounded set of existing canonical source files without pretending to
solve repository-wide Graph semantics, raw working-tree materialization, or
crash/power-loss durability.

## Decision

Semantic Workspace Transaction v1 owns a hidden managed control tree under
`.semaprax-workspace`. A complete source generation is immutable and named by
the domain-separated digest of its canonical manifest. One canonical `ACTIVE`
file selects the generation. Cooperating readers acquire the permanent shared
lock, authenticate `ACTIVE`, and read only the selected generation. Writers
hold the permanent exclusive lock, prepare and authenticate the complete
candidate generation, publish it without replacement, and replace only
`ACTIVE` after two final checks.

Initialization is the separate authority that creates generation zero and the
first `ACTIVE`. Ordinary apply never rewrites the original source files. The
wire, limits, reader contract, diagnostics, and nonclaims are frozen in
[Semantic Workspace Transaction v1](../SEMANTIC-WORKSPACE-TRANSACTION-V1.md).

## Consequences

- Cooperating managed readers observe one complete old or new generation; no
  sequence of per-source renames is exposed to them.
- Raw paths, Git, editors, build tools, and noncooperating readers are outside
  the atomic-visibility claim.
- A fully authenticated candidate can remain after a rejected pre-pivot apply.
  Bounded staging and retained-generation residue is inventory, not a selected
  commit.
- Failure after the `ACTIVE` replacement is an explicit `SPX-I212` ambiguity;
  the implementation does not silently roll back a possibly selected new
  generation.
- Garbage collection, rollback policy, flat materialization, repository
  migration, and power-loss recovery remain future protocols.
- Existing single-file Patch/A0/Impact/Review/Repair/Target/Evidence protocols
  are unchanged. Workspace v1 embeds admitted patches per file but grants those
  artifacts no new authority.

## Rejected alternatives

### Sequential per-file replacement

Rejected because portable readers can observe a mixture of old and new files.
Recovery also cannot infer a unique committed set from an interrupted rename
sequence without another publication record.

### Filesystem-specific directory exchange

Rejected as the public contract because portable Rust and the supported host
matrix do not expose one identical no-replace/exchange primitive with the
required semantics. It would also overstate network and hostile-filesystem
behavior.

### Git commit as transaction authority

Rejected because Git state is not the compiler's authenticated live source
authority, ordinary working trees can be dirty, and editor/raw-file readers
would still observe path-by-path materialization.

### Database or opaque package as canonical source

Rejected for this tranche because readable `.spx` remains the canonical Git
projection. The managed tree is an opt-in publication layer, not a replacement
language storage format.

### Widen single-file A0

Rejected because it would conflate two trust boundaries and risk changing the
frozen single-file protocol. Workspace v1 has its own permanent lock,
generation inventory, and `ACTIVE` publication authority.
