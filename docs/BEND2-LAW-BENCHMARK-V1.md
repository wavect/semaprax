# Bend 2 law benchmark v1

Issue #392 compares law-preserving development under an explicitly pinned
local configuration. The owning runner is
[`benchmarks/bend2-law-v1/run.py`](../benchmarks/bend2-law-v1/run.py) and its
manifest is the only committed task inventory.

## Admission

A result is admitted only when the command file pins accessible Git roots for
both Bend and SEMAPRAX, the observed heads equal those declared roots, Bend is
at `947db722640c86247849343657bf2f7ef01cb7f1`, and every one of five execution
paths is declared separately. The five paths are ordinary Bend checking,
Bend `--verdict`, SEMAPRAX SMT, external Lean, and SEMAPRAX runtime/test.
Their receipts are never merged.

Each cell declares its numeric semantics and equal law inventory. The runner
executes the success subject once cold and at least thirty warm times, retains
all raw warm samples with p50/p95, and runs every seeded law-gaming control.
Any accepted attack fails that execution path. Missing tools, timeouts,
identity drift, unsupported targets, and mismatched domains are unavailable
or failed outcomes; they have no score and cannot establish a win. Smaller
runs need `--pilot` and remain pilot evidence.

## Scope and remaining evidence

The manifest has all six required cells: scalar contract bug, balance
transfer, list theorem, law-preserving refactor, law-breaking agent edit, and
project incremental edit. It binds sort permutation/multiplicity plus
sortedness, and balance conservation plus intended state change, closing the
empty-sort and no-op-transfer loopholes at the harness boundary.

Each cell has a digest-bound checked-`u32` input/output corpus under
`benchmarks/bend2-law-v1/fixtures/`. It is language-neutral because the
reviewed SEMAPRAX scalar profile does not admit `u32`; replacing it with its
smaller `i32` profile would make the comparison unequal. Bend's pinned `U32`
implementation/proof sources are also unavailable until its pinned executable
and checkout are provisioned locally. These are explicit unavailable cells,
not successful evidence.

No result is committed. Closure still requires matched executable source and
proof fixtures for each declared `u32 checked` domain, a quiet-host
30-repetition run with both pinned checkouts, and agent trials with ten
independent fixed-budget trials per admitted language/model plus token/cost
event provenance. Runtime throughput and GPU scaling remain a separate family.
