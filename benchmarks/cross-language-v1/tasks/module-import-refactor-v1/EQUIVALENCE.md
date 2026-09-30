# Task equivalence: `module-import-refactor-v1`

This task measures a multi-module invoice calculation refactor. The public entry point must import a helper for whole-subtotal tax calculation; hidden execution keeps helper and candidate modules unchanged and overlays only assertions.

## Problem and oracle

`invoice_total(price, quantity, tax_rate, shipping)` computes, in order: `subtotal = price * quantity`; `tax = floor(subtotal * tax_rate / 100)` for nonnegative integer inputs; then `subtotal + tax + shipping`. Tax is calculated once from the whole subtotal, and shipping is added after tax. The candidate imports `tax_for_subtotal` from a separate helper module. This module arrangement is a fixture-level refactor requirement; behavioral tests alone do not prove that an import was used. A candidate that rounds tax per item or includes shipping in the taxable base passes simple public cases but fails hidden fractional-tax/shipping cases.

Public vectors use exact/simple cases. Hidden vectors include fractional whole-subtotal tax with nonzero shipping, distinguishing a helper that rounds per item or taxes shipping. Every vector is within the exact intersection of Rust `i64` and JavaScript safe integers (`0..=9_007_199_254_740_991`); concrete inputs are nonnegative and far below that ceiling.

## Independent hidden overlay

Rust and TypeScript hidden phases overlay only their assertion entry module; copied public helper and candidate modules remain under test. SEMAPRAX hidden overlays only executed `src/app.spx`, preserving copied `src/helper.spx` and `src/candidate.spx`. A wrong imported helper is intended to pass public vectors but fail hidden assertions; that claim requires the runner's actual negative-control execution. Deleting public assertions cannot make hidden acceptance pass because hidden assertions call the same copied modules.

This directory is fixture-only evidence until the existing runner is invoked with registered adapters; no trial or language comparison is claimed here.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. Python and Java preserve a
real cross-file import: `candidate.py`/`Candidate.java` import
`helper.py`/`Helper.java` (Python's script directory is always on
`sys.path`; `javac Main.java` auto-discovers and compiles both from the
same directory), and the hidden overlay replaces only `digest.py`/
`Main.java`, leaving both helper and candidate modules unchanged, exactly
as the Rust/TypeScript ports do. C simulates the same split with
`#include`, since this suite's C adapter compiles only `main.c`:
`candidate.c` `#include`s `helper.c`, and `main.c` `#include`s
`candidate.c`; only `main.c` is overlaid for hidden. Swift's fixed
single-file `swiftc main.swift` invocation admits no file split, so
`Helper`/`Candidate` are kept as distinct namespaces within one file
instead, repeated verbatim between the public and hidden `main.swift`.

Each port was authored independently against this file's own
`subtotal`/`tax`/`shipping` ordering contract and the Rust/TypeScript
references, not transliterated line-by-line, and was independently
compiled/run against the public and hidden vectors above. Each was then
checked against a deliberately mutated helper call that computes tax
per-item (`quantity * tax_for_subtotal(price, tax_rate)`) instead of once
over the whole subtotal — the "rounds tax per item" bug this file's
"Problem and oracle" section names. That mutant passes all three public
vectors (the per-item and whole-subtotal tax happen to coincide there) and
fails all three hidden vectors (`70`/`44`/`67` instead of the correct
`71`/`48`/`77`) in all four languages, confirming the hidden vectors are
non-vacuous.
