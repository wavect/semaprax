# Recovery constructor funding design v1

Audience: compiler contributors, tool authors, and reviewers.

Status: design and source-authored executable boundary controls; not a funded route.

Subject: source diagnostic recovery at `182482621259ce5ad6ed1538b641850956754ee5`.
This document records the explicitly authorized design fallback after source review.
It changes no source-verifier behavior, constructor payment, quota, diagnostic,
public API, completion status or original receiving body. No Cargo, build, test,
Clippy or full-profile gate ran in this lane.

## Exact current owners

`source_verify/recovery.rs` owns the lazy lexical rows, borrowed names and
annotation-expression refusal views. Its `signature` constructor rechecks each
affected parameter once per invocation after an existing `SPX-T001` and matches
all six authoritative diagnostic fields. Parameter binding/type/mode equality
and drop admission run first. This recheck is distinct from per-use lookup.

| Storage or work | Acquisition/traversal owner | Current payment |
| --- | --- | --- |
| Scope rows | `Recovery::ensure` uses `Vec::resize_with`; `fork` resets a reused row and drops its former map | None |
| Refused names | `signature`, `publish`, `refused_annotation` insert into a row's `BTreeMap` | None; borrowed names do not fund nodes |
| Annotation identity entries | `reads_value` inserts into `expressions`; `begin` removes stale entries; `view` follows only transparent tails | None |
| Signature parameter set | `signature` lazily collects authored type-parameter names into a `HashSet` | None; an empty set has no backing |
| Signature temporary diagnostics | `signature` owns and reuses a scratch `Vec<Diagnostic>`; clear/drop retires previous payloads | None |
| Declared-type recheck | Shared `declared_type::check_declared_type` owns a pending vector and ordinary dynamic messages, paths and optional help | None for this recheck |
| Ancestor lookup | `Recovery::read` visits one lexical row at a time; test-only `visited` counts actual rows | No work debit |
| Refusal selection | `publish` scans only the new diagnostic frontier; signature compares complete original refusals | No work debit |

`IterativeVerifier::new` acquires signature recovery before constructing its own
scope vector. `iterative/run.rs` owns frame traversal and receiving lifetime;
`iterative/enter.rs`, blocks, operators, matching and match_arms own scope entry,
publication and lookup sites. Detached closure entry deliberately passes no
recovery parent. Ordinary successful forks acquire no recovery directory.

The paid identity sibling in `hir/identity_resolution.rs` runs ordinary
`verify_presenting` before its explicit identity callback. That callback does
not pay recovery. Builder `capacity/hir_pre_resolve.rs` computes `source_phase`
and takes its maximum with other phase estimates; a forecast is neither an
allocation reservation nor a recovery-constructor payer. Neither owner is
changed by this design lane.

## Why this batch stops at design

A refused payment has no faithful representation in the current signatures:

- `reads_refused -> bool`: false selects a new independent `SPX-T202` at its
  caller; it cannot mean that lookup storage/work payment failed.
- `reads_value -> bool` and `view -> Option<RefusalView>`: absent facts change
  expected-type suppression or inference. Payment cannot erase an existing fact.
- `fork`, publication, clear/begin and inherited binding methods return `()`;
  `signature` and `IterativeVerifier::new` return `Self`. Their callers cannot
  propagate a separate capacity refusal before acquisition.
- `run -> Result<_, Diagnostic>` carries source semantics. The receiving
  `check_expr_with_helpers_view` appends its `Err` with the authored path and
  returns `(None, None)`. Sending payment refusal through that arm publishes a
  new source diagnostic and loses its distinct outcome.
- The signature recheck calls the shared declared-type checker. It constructs
  diagnostics, including owned payloads, before comparing them to the original
  refusal. Charging only its outer scratch vector leaves real inner storage
  unpaid. A paid sibling or an exact borrowed refusal matcher needs independent
  full-field parity, forged-span and duplicate-parameter controls.

This is an API and failure-selection migration across the declaration and
expression receivers. It is not safe to implement by checking the active budget
and returning false/None, or by installing a paid directory that is not reached
by the real verifier. Scoped recovery payment alone could be an honest partial
route if explicitly described and connected; it would still not establish full
source-verification payment. No such route is added here.

## Concrete disjoint next implementation batch

The next batch can own recovery, its necessary expression/declaration caller
plumbing and its declared-type recheck sibling. It must coordinate the private
identity-resolution entry when connecting an actual paid call. It must leave
DeclarationId representation, ReplayBudget, cleanup and builder forecasts to
their owning lanes.

1. Define a private, allocation-free recovery payment refusal and a separate
   failure enum for source verification. Ordinary entry points retain their
   complete diagnostic receiver. The actual paid entry supplies the existing
   cumulative payer explicitly; absence of a payer retains ordinary behavior.
   Specify source-error versus capacity-refusal selection before changing it.
2. Make recovery construction, mutation and work-bearing lookup fallible.
   Carry payment failure through `try_new`, frame handlers, the expression
   receiver and the declaration receiver without conversion to an absent fact,
   invented type/binding, or owned source diagnostic. Previously selected
   refusal remains sticky. Drop acquired scratch on all exits; publishing an
   incomplete diagnostic list must not imply successful source verification.
3. Reserve scope-vector backing before growth. Price each real acquired layout,
   including replacement overlap, with checked arithmetic. Rust `BTreeMap`
   has no fallible allocator/reserve frontier: either use the repository's
   independently witnessed conservative node admission, or migrate to a
   compiler-owned fallible borrowed directory with complete old ordered lookup
   parity. Per-entry logical size and borrowed name length are not node backing.
   Replacement/removal must not manufacture credit or refunds.
4. Remove or fund the signature diagnostic recheck's actual inner allocations.
   A borrowed matcher must reproduce all six fields, traversal and original
   first-refusal choice; a fallible diagnostic sibling must fund vector slots,
   pending storage and dynamic message/path/help payloads before construction.
   Keep current binding/drop/type checks and exact forged-span/arity defenses.
5. Keep work and storage receipts distinct. Count actual ancestor rows,
   transparent-tail steps and signature/frontier comparisons; any work price
   needs an explicit existing payer contract. Never convert these counters into
   storage authority, historical debit credit, RSS or agent-token savings.
6. Qualify the actual paid production entry with independent original/current
   semantics, exact/one-short constructor and work admission, already-sticky
   refusal and retries, late failure, receiving Drop, unchanged limits and
   complete metadata. Ordinary zero/sticky behavior is compatibility evidence,
   not proof of paid refusal. Do not relabel the controls below as funded tests.

The design intentionally contains no unused production trait, helper, paid.rs
stub or synthetic authority route. The next migration must revise the ordinary
boundary controls only when its explicitly scoped paid sibling is real;
ordinary compatibility controls remain independent.

## Source-authored boundary controls

The connected `source_verify::recovery_tests::funding_boundary` child owns eight
controls. They use the actual production recovery constructor/methods, complete
ordinary public source verification and independent saved/current iterative
receivers. Parsing, original diagnostics, held immutable prelude and active
budget setup remain outside constructor observation where appropriate. Every
allocation observation completes receiving Drop; requested Layout bytes are
not malloc usable bytes or RSS.

- Zero budget still acquires scope/name storage and visits the full lexical
  ancestry without issuing a reservation.
- A previously sticky one-byte refusal remains byte-for-byte unchanged after
  recovery acquisition and zero/larger retries.
- Exact observed requested allocation bytes and one-short both admit unpaid
  recovery; these observations cannot authorize a paid frontier.
- Annotation identity nodes remain real unpaid acquisitions.
- Signature recheck still binds the exact known refused parameter and releases
  its scratch/directory through Drop under a zero budget.
- Ordinary zero, already-sticky and unlimited active budgets preserve every
  diagnostic field for the explicitly bounded missing/annotation/signature
  fixtures. This does not assert that every unrelated verifier feature ignores
  active budgets.
- Full public verification of a missing/alias fixture allocates and drops real
  storage under zero budget, while retaining its complete root diagnostic.
- Saved/current full iterative receivers retain their separately stated
  intentional dependent-read delta and actual full Drop under an active budget.

No new top-level test binary is added. The builder's real
`verifier_source_inventory!` text assertion join includes the child. Existing
original receiver code, raw literal controls, module-size and source-coverage
baselines are unchanged. Exact pending selectors and review receipts live in
`evidence/recovery-funding-boundary/`; none is passing execution evidence.
