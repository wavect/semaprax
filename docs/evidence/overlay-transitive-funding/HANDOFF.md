# Common production walker acquisition repair

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Base17efd. Source-only batch: no builds/tests/Clippy/full gates executed. Rustfmt and diff whitespace checks complete. Full original source snapshots match base git objects, and all original walk/presence function bodies remain compiled unchanged. The complete original mutable/borrowed validate_uses bodies are independently compiled cfg(test) controls. The mutable source contract keeps the immutable original digest and proves exact full-current equality after precisely three named fallible walk hooks; no other byte is waived. New descendants join existing builder and HIR source audits.

The actual receiving failures14/18 already fail on current=false with4374897 paid and5372235 allocated, all freed. Resolution and validation both contribute; no whole deficit is attributed to this patch. The concrete unpriced route is function_value::walk's Vec-backed pending root/children traversal, repeatedly called by mutable/borrowed HIR use validators before the no-function-values gate. Its test-only funded twin did not cover production.

The new production child has32 inline borrowed pending entries and an unbounded fallible spill. Inline storage acquires no heap; only actual full replacement heap backing is charged, before acquisition. Every setup/push authenticates sticky reserve0. Spill layout overflow, allocation failure and allocator disagreement refuse; excess backing is charged and never silently used. Root pop, authored child order, closure capture-only dispatch, template metadata short-circuit and complete function-body scans match the original. No traversal cap, forecast, credit, observer exclusion or caller-owned allocation was added.

Migrated exactly three mutable and three borrowed fallible scans plus the Result-returning function-value presence check at its original false early-return position. Old infallible API receivers, target-universe/closure inventories and inner mutable argument-staging scans remain unchanged and may retain independent open acquisition work. This batch does not claim whole overlay funding, application fit or closure of the separate50-byte function-scope deficit. The unchanged full original overlay physical assertions and failed function-scope owner remain pending selectors.

New authored controls compare complete old/current traversal order and presence, independent complete closure-use verdicts (including original active borrowed/mutable fixtures and hostile mutable parameter), actual spill physical allocation/free/Drop, exact and one-short limits, sticky refusal, and literal old/current scalar acquisition. All still require actual combined qualification.

Claude received11219bytes of complete definitions and returned a bounded drafting review in69.4s: actual claude-sonnet-5-5 / firstParty, listbasis$0.1131335, actual billing unknown. It proposed the faithful inline/spill route, old receiver retention and Result propagation; its assumed Budget API was mapped to the real sticky active ledger. The inline price is actual heap-zero, not a discount against a phantom Vec allocation. No code executed by Claude.
