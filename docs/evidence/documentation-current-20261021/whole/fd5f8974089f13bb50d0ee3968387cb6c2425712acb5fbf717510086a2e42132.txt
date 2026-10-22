# Ordinary skeleton constructor funding (#856)

Source checkpoint from a82e00cc83f99b16249181c45710d962598577db. No Rust
compilation, runtime test, Clippy, allocation observation or savings is claimed.

Actual344 owners140/166/172/177/183 all failed the unchanged assertion
`alloc.allocated <= paid`. Their complete original logs are retained here.
Each owner constructs SkeletonWork inside the allocation observer before its
first seed reservation. The old constructor performs renewal, moving-string
and condition walks that allocate Vec/map/set scratch before any physical
admission. The constructor—not the later prefix transfer—is the shared gap.

The ordinary constructor now prepares its original ReplayBudget refusal text
and514-slot clone census cursor first. Zero-budget bootstrap failure therefore
returns the original empty inline Diagnostic without acquiring message backing
or collector scratch. New fallible-result, closure and explicit payer receiver
layouts are reserved by their concrete types before construction. The existing
explicit-payer funded collectors are then called in original order: renewal
families/merge precedence, moving strings, condition reads, lexical presence,
and owner candidates. Each actual scratch growth, node and clone payload is
paid on this same ReplayBudget before acquisition. No work ceiling, byte cap,
forecast, transfer algorithm, clone encoder or ordinary sink is changed.

Only the13 real funded dependency modules become production-visible. Scripted
test payers, work-oracle modules, unused single-site wrappers and complete old
ordinary collectors remain test controls where no production caller remains.
All original helper bodies and helper-census WORK/DEPTH ceilings are unchanged.
Payer Refused(Diagnostic) is moved out unchanged, retaining all six fields and
the selected receipt; layout failure is propagated, never an empty inventory.
The historical census work/depth absence remains exactly its original case.

The complete previous with_index body remains executable under cfg(test) as
with_index_before_funding. New controls compare old/current collector answers
on every declaration and generic instance, with and without a held index.
The constructor's own zero/exact/short/sticky/Drop owner separately executes
the old zero-budget unfunded behavior as a historical witness. Where failure
comparison is lawful, it compares all six Diagnostic fields against an
independently executed original bootstrap; it does not pretend the old
constructor failed at zero. The five original failing tests retain every
statement/assertion and budget; only allocation assertion diagnostics change.

All13 promoted modules are bound by raw includes in all three authority
inventories. HIR's union was already complete; missing workspace/native entries
are appended without altering any original guard byte or include order.

Complete exact Git preimages and whole-source inverse proofs are retained for
every changed preexisting path. Verify source custody only:

```sh
python3 docs/evidence/skeleton-constructor-funding-856-20261016/verify-source.py --repo /absolute/checkout
```

After joining, refresh the complete current endpoint views (including these
changed roots and inventory guards), then regenerate truthful production/test
authority partitions. Original tables, predicates and original packets remain
immutable. New production visibility is a real implementation change; it is
not production-source equivalence to a82e and does not reuse its old Clippy
qualification. `pending-selectors.json` lists the new constructor controls and
the five unchanged failing owner selectors for consolidated qualification.
