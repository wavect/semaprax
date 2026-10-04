# Collection law pack v1

This pack states **sortedness and exact element multiplicity together** for
insertion sort over immutable `List<i64>`. The source uses comparisons and list
constructors only; element values do not undergo arithmetic. The proof profile
is `semaprax.collection-sort-i64.v1`, with source semantics
`semaprax.checked-i64.immutable-list.v1` and proof module
`semaprax.collection-sort-proof-module.v1`.

## Review the law before reviewing the implementation

A sorted output can be empty, so sortedness alone is too weak. Length plus
sortedness is also too weak: `[1, 2]` and `[1, 1]` have the same length and both
are sorted. For every element value, its number of occurrences must be identical
in the input and output. The fixed generated laws are:

- `insert_permutation`: inserting one value preserves every existing element
  and adds precisely one occurrence of that value.
- `insert_sorted`: insertion preserves an already sorted list.
- `sort_sorted`: every pair of output elements in forward order is nondecreasing.
- `sort_permutation`: the output is a permutation of the entire input.
- `sort_multiplicity`: for every value, the output count equals the input count.

[`sort.spx`](sort.spx) is the reviewed implementation. Its small standalone
`main` sorts `[1, 2]` and encodes the first two output elements as a decimal
pair (12 for the correct body, 11 for the duplicate mutant, and −1 for empty
output). The universal proof covers `insert` and `sort`; the sample entry is
explicitly outside that theorem inventory. The separate authored
[`lemmas.json`](lemmas.json) supplies proofs under those compiler-fixed theorem
statements; it cannot replace the statements with weaker ones.

## Correct, broken and repaired source

The owning gate reads these exact saved files and the same proof module:

| Source | Owning result under unchanged laws |
| --- | --- |
| `sort.spx` | All five laws prove. |
| `mutants/empty-output.spx` | The nonempty sort branch returns `[]`; sortedness is true, but permutation/multiplicity fail. |
| `mutants/duplicate-element.spx` | Insertion replaces a head with a second copy of the inserted value. For input `[1, 2]`, output `[1, 1]` has unchanged length and is sorted, but permutation/multiplicity fail. |
| `repaired.spx` | Restores the implementation body without changing any law or proof-module bytes. |

The installed Lean gate passed 2/2, including source admission and both mutant
controls. Each bad proof returned `SPX-LW140` and no certificate. This is a
refused certificate, not a successful certificate with a warning. Separately, the kernel checks a concrete counterexample for each bad
source: input `[1, 2]` remains sorted but the output has the wrong count of 2.
This bounded witness establishes falsity of the multiplicity law on that input;
a failed authored proof alone would not establish it. The gate also refuses replay of a previously correct certificate on a
mutant, changed proof module, changed semantics/profile version, or forged
statement. Repaired source produces the original certificate.

## Run locally

No editor or hosted service is needed. Provision the repository's pinned
**Lean 4.34.0** explicitly (including its bundled `Std` library) and Rust/Cargo
from the repository development requirements. The test never downloads a solver
or toolchain and never selects a binary from an implicit path.

```sh
cargo run --locked --offline -p semaprax -- check examples/law-packs/collection/sort.spx
cargo test --locked --offline -p semaprax --test language collection_law_source_ -- --nocapture
SEMAPRAX_LAW_LEAN=/absolute/path/to/lean \
SEMAPRAX_LAW_LEAN_VERSION="$(/absolute/path/to/lean --version)" \
cargo test --locked --offline -p semaprax --test language \
  collection_law_pack_pinned_lean_rejects_empty_and_duplicate_outputs_and_repairs \
  -- --ignored --nocapture
```

The physical test uses the ordinary checked-source/HIR/graph route, the
`proof_export::list_sort` library API and the existing bounded held-process
provider. The source translator visits each admitted body expression. It does
not replace an arbitrary sorting body with a known-good sorting definition.
Bodies outside its pure, direct structural recursion/comparison/list-constructor
profile receive `SPX-LI015`; an unproved source shape does not inherit a theorem.

## Domain, assumptions and trust

Lean's inductive lists are finite but unbounded in mathematical length; counts
use `Nat`, and exact signed elements embed into `Int`. Structural recursion on
the destructured tail is checked by the kernel. These facts establish totality
of the mathematical functions and the universal laws for their denotations.
They do not establish that finite machine resources suffice for every list.

The runtime list carrier has bounded length and may refuse allocation or depth.
Its checks and native/Wasm lowering remain separate executable evidence. The
trusted proof boundary includes Semaprax source/HIR validation, this closed
translation, Lean elaboration/kernel and the installed pinned toolchain. The
report records the actual axiom set; only the standard Lean axioms `propext`,
`Quot.sound` and `Classical.choice` are permitted. No custom list axiom, finite
enumeration or assumed sorting result replaces induction.

Certificates bind canonical source, translated definitions, full proof module,
exact generated Lean document, semantics/profile versions, coverage and axiom
reports. All dependent evidence must be regenerated after those inputs change.
The library certificate is source proof data; it grants no publication, process,
filesystem or other runtime authority. This pack does not yet claim selected
Project strict-law attachment or a public list ABI.
