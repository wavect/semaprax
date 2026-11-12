# Factored singleton successor targets v1

Status: private source contract with local executable evidence for OPT #870.

Audience: compiler contributors and reviewers.

This source-only optimization is pending combined executable qualification.
Canonical label/child ordering, route multiplicity, checked overflow, signature
algebra, physical refusal ownership and all existing replay-work ceilings remain
unchanged. No application recovery or universal allocation/peak benefit follows.

A successor label is created by its first actual move and therefore needs no
empty target variant. Its One(child,weight) stores that first checked zero-plus
weight directly. Repeated One children execute the original checked_add inline
in the receiving canonical function, before any later target acquisition.
A second distinct child invokes promotion: full new initialized/receiving and
Result/map/tuple/Option layouts and BOTH target-node acquisitions precede EITHER
insert. A refusal leaves the original One unchanged; no quota refund follows.
Many uses the original ordered tree insertion/checked-add loop. Repeated Many
children do not acquire another node. Sorted finish and canonical IDs are intact.

The label BTree node acquires its full NEW Targets value layout, not the old map
layout or their difference. New map/enum/query/initializer/Result/receiving and
finish iterator carriers are explicitly acquired; old headers grant no credit.
One's output uses one exactly precharged tuple slot and the existing paid Rc
backing. The original weighted_successor(BTreeMap) function and width0..4 physical
contracts remain complete and unchanged. No work request/unit/cap is discounted.
Targets is ephemeral inside the immutable function and current budget borrow;
its enclosing Memo still binds exact function/budget/current ledger and sticky
failure. A storage variant conveys no authority or new paid provenance.

Retain the complete immediate predecessor and all older full receivers. Owning
qualification includes sorted widths0..4, singleton/repeated/promotion/overflow
and competing order, complete signatures/states/six-field diagnostics, same work,
actual reserved/allocated/highwater/whole Drop, every actual acquisition with
exact/short/zero/sticky boundaries and foreign/restored/stale owner denial.
Measure genuine baseline tuples and disclose any many/repeat/layout regression;
removing one singleton tree does not establish total net savings. Original retry
and normalizer application owners must run unchanged with outcomes disclosed.

## Receiving-layer successor (source only)

The production successor function now performs One construction, promotion and
finish inside its existing Result receiver. The complete immediate831 methods
remain test-only independent controls; they do not qualify the new production
boundaries by themselves. One removes its extra Result<Targets,Diagnostic>
method return, promotion removes its Result<(),Diagnostic> method return, and
finish removes the consumed method argument/return layer. All actual remaining
initializer/map/Option/tuple/Vec/Rc controls remain acquired. Both target nodes
still precede either promotion insertion, and repeated-child overflow still
precedes any later acquisition.

One materializes an exact one-entry Vec from a one-entry array, whose complete
initializer is separately paid; there is no Once iterator or fallible weighted
helper on this branch. Its Rc backing is reserved before Rc::new, after the
existing vector acquisition. Many keeps its fallible weighted helper and pays
its real map argument and Result receiver. An empty successor map now bypasses
owning-iterator construction altogether, after the unchanged zero-byte output
vector authentication; only actually absent iterator/tuple construction fees
are omitted. No refund, shared-header credit, fee exemption, work-unit change or
new budget is introduced.

Complete frozen831 receiver, helpers and all seven original controls remain
archived/compiled/byte-guarded. New controls exercise actual inlined successors:
complete signatures/states/overflow/work, exact one-entry capacity, real old/new
reserved/allocated/highwater/whole Drop tuples, and every byte cut through One,
promotion and finish. The shared Memo's original foreign/restored/sticky owners
remain selected. No measured improvement or application fit is established
until those controls execute with unchanged application owners.

## Empty canonical signature receiving (source only)

A canonical Exit with no successors retains its original signature-work debit,
preflight, held payer authentication and insertion ordering. After explicit
admission of its actual branch and key constructors, it builds the real empty
signature directly. The original zero-length signature-vector authentication
still precedes dictionary lookup; signature insertion precedes state insertion.
It constructs no child-ID query, scratch-growth Result, child iterator or Zip.
No corresponding absent constructor is charged, and no old charge becomes a
credit. Nonempty signatures execute the complete existing borrowed-query path;
they also pay the actual new branch receivers, so unfavorable wide/unique cases
must remain visible. No weighted-state layout, graph identity, work ceiling,
physical cap or refusal authority changes.

The complete immediate d962 receiver remains independently compiled in
`before_empty_signature.rs`, with all its ordinary/general branches intact.
The a82 log17 paid regression is historical: d962 already includes newer arena
changes, and its actual current gate outcome is pending. New tests compare
complete IDs/states/six diagnostic fields/work frontiers, exact/short/zero paid
limits, sticky foreign/restored owner denial, actual heap/paid/peak and full
Drop against that immediate receiver. The original strict paid gate and wide/
unique controls remain unchanged. No executable gain or application recovery is
claimed before a fresh combined qualification.

## Composed paid Many successor (OPT870)

The ordinary canonicalizer keeps the preceding first-child and repeated-child
One bodies. A distinct second child on a paid invocation now constructs sorted
flat Many backing in `canonical_shared/paid_many.rs`. Both actual unit-closed
and general weighted successor callers first acquire the helper arguments,
return/receiving enums, vectors, tuples, arithmetic Options and nested Results.
The existing fallible `reserve_vec` owner authenticates before acquisition,
charges the complete geometric capacity before allocation, and owns allocation
failure and capacity mismatch. It charges spare capacity before either tuple
is inserted. Publication replaces One only after successful promotion.

The flat receiver uses sorted insertion and prepays the bounded binary-search
callback roles. A present key uses checked addition before mutation. A new key
acquires backing before insertion. The caller prepays the helper, Result and
borrowed vector roles before forming the borrow. The list holds at most eight
distinct targets. The next distinct target reserves the complete ordered tree,
then moves the flat entries and inserts the new target. Finishing a flat list
moves the paid vector into the existing immutable Rc owner after paying its
receiving boundary.

Both routes use the same per-move logical work tariff. The flat search and
shifts fit within a fixed eight-row bound and add no payer-specific logical
debit. The unobserved legacy route retains tree promotion, insertion and
finish. The paid representation adds receiving costs and may retain geometric
spare capacity. The measurements below show the resulting charged bytes and
allocator requests separately.
The complete immediate51c1 Canonicalizer, terminal implementation and production
comparison caller compile in `before_paid_many`, using the original Targets and
source-bound unchanged helpers. All115 existing helper/test sources and both
complete source preimages remain sealed. Controls compare the complete original,
immediate and composed algorithms; actual old/current tuple and iterator layouts;
unit/general promotion; sparse/repeated/overflow algebra; all promotion byte cuts;
zero/exact/one-short complete-owner limits; full six-field diagnostics; sticky,
foreign-function/budget/ledger ownership; and physical allocations through Drop.
The actual HIR/plan comparison caller and full lexical trace fixtures remain
separate controls from direct automaton coverage. CLI, ShiftSim and normalizer
owning gates, scoped Clippy and these focused bodies require later coordinated
qualification. Independent incoming flat tests alone establish no adoption.

### Local qualification at `dbd41b589`

The serial `paid_many` and `singleton_target_tests` lib run on macOS arm64
passed 28 checks and failed three. The passing component controls included
exact, one-short and zero allocation limits, weighted overflow and full Drop.
The general canonicalizer comparison at unit weight measured the following.
Allocator bytes are cumulative requested bytes observed through Drop.

| Distinct targets | Paid bytes, inline to composed | Allocator bytes, inline to composed | Allocations, inline to composed | Work, both |
| --- | ---: | ---: | ---: | ---: |
| 0 | 13,194 → 13,214 | 1,181 → 1,181 | 10 → 10 | 9 |
| 1 | 20,726 → 19,146 | 3,021 → 3,013 | 27 → 26 | 41 |
| 2 | 22,670 → 22,999 | 3,381 → 3,181 | 31 → 29 | 52 |
| 4 | 24,622 → 30,055 | 3,781 → 3,613 | 36 → 35 | 74 |
| 17 | 36,270 → 56,365 | 7,581 → 7,797 | 59 → 61 | 217 |

The two whole-source failures first encounter
`src/cleanup_plan/replay/factored/oracle_order.rs`. Its 31,188-byte frozen
input from `51c1dfc49` still matches the sealed helper digest
`a5b274eb811d25ba6b1aa0b78ef108859c7f37bef32fb9236a51b816011f4011`.
The historical reader at that commit failed to acquire those bytes. The
source-custody repair binds five complete helpers from that same sealed recipe
to current compiled and physical inputs. Both source checks pass with all 115
helper digests and the original forged and malformed-input cases.

The full comparison stopped at its first unobserved work check. The saved
caller left 31,998,534 units and the shipping caller left 31,998,418.
Temporary test instrumentation measured all 116 units in the shipping HIR
memo release for that case. Partition validation charged 79 units and
private-key release charged 37. The independent PLAN-only work-cut check and
the release refusal check both passed on the custody repair. The complete
cold caller check still failed its allocation tuple equality. Its pre-release
caller measured 6,206 allocations, 352 reallocations and 960,776 requested
bytes. The current caller measured 6,222 allocations, 364 reallocations and
956,584 requested bytes. Both callers returned the same verdict and released
all observed backing. Complete work-cut and matched application comparisons
remain pending.

The selected LogLens CLI, typed Catalog, and typed ShiftSim application owners
passed. All 13 Catalog Normalizer functional owners passed, and its
12-module graph built under the current 128 MiB builder cap. The
older 64 MiB Catalog and ShiftSim controls both refused with SPX-G171.
The application verdicts do not attribute a debit change to the flat
successor receiver.

### Complete source comparison after the custody repair

The local macOS arm64 qualification passed 35 focused Rust checks, 29
source-overlay checks and scoped production Clippy. The Rust run included
the complete 115-helper custody check and the Catalog Normalizer builder.

The frozen inline caller and the independently compiled caller from before
memo release agreed on the original work cuts and diagnostics. The shipping
caller agreed at its own cuts after the separately measured release debit.
The shipping caller refused when it received only the original exact work
allowance. The fixtures retain the independently enumerated PLAN multiset
and the original Copy-latch authority refusal.

| Source probe | Inline work | Shipping work | Memo-release work |
| --- | ---: | ---: | ---: |
| Scalar while | 1,466 | 1,582 | 116 |
| Generated Copy latch | 3,391 | 3,637 | 246 |
| Borrowed record latch | 2,546 | 2,721 | 175 |

| Admitted physical probe | Paid bytes, inline to shipping | Allocator bytes, inline to shipping | Allocations | Reallocations | Peak live bytes, both |
| --- | ---: | ---: | ---: | ---: | ---: |
| Scalar while | 788,434 → 866,579 | 150,429 → 150,421 | 502 → 501 | 96 → 95 | 77,189 |
| Borrowed record latch | 1,214,568 → 1,330,409 | 241,883 → 240,771 | 901 → 895 | 163 → 163 | 82,861 |

Both admitted probes exercised the full authenticated comparison caller and
settled all observed backing. Each caller kept its own exact, one-short and
zero physical limits. The shipping borrowed-record probe made four unit
and two general paid promotions. The scalar while probe made none. On the
paid route, total work moved from 23,469 to 23,548 for the scalar while and
from 1,250,041 to 1,250,161 for the borrowed record. The corresponding
skeleton work stayed at 11,541 and 716,677.

The larger invoices are cumulative reservations for the current whole caller.
They include changes since the saved caller and do not isolate flat successor
storage.

## Direct selected compatibility authentication closure (source only)

The focused452 direct singleton negative exposed that zero-byte payer/work checks
do not establish a function owner. The actual ordinary caller already binds its
Memo; the direct selected compatibility receiver now uses a funded setup anchor
inside its selected physical ledger. It binds the exact function pointer and
existing held scope before any payer query, logical work or output. General
multi-function Session spending remains unchanged. The ordinary ReplayBudget
layout and constructor bodies remain exact; selected ledger layout/capture costs
are additional and must be reported against the independently retained old type.

The selected-successors test adapters now perform existing Memo identity/sticky
preflight before acquiring an invocation. Their whole prior bodies remain
independent negative controls, including original payment-before-scope failure.
Production held-receiving logic is unchanged. Full before inputs, original
assertions and new same-ID/scope/restored/ledger-layout controls remain required.
No executable correctness, reservation improvement or application fit is asserted.

## Selected outer-label route correction (OPT831/870, source only)

The observed complete singleton receiver paid more after the selected Groups
transport even though it allocated less. General empty labels also acquired
Groups entry and transport controls and violated the unchanged exact small
fallback contract. Selected replay therefore uses the already paid outer label
tree and inlined One targets. Its old label-node, initializer, iterator and
publication payments remain real and unchanged. Distinct children still enter
the composed paid flat receiver; no promotion/search/finish tariff is removed.
The Groups representation, macro and complete immediate canonical producer are
retained independently under test cfg. Their original receiving banks remain
unchanged; they supply no credit to current code. All preceding controls remain
executable. The new unprojected source owner distinguishes the current fallback
from the whole historical Groups producer. Strict831 saving and exact870 small
fallback assertions are unchanged and require actual combined execution.

## Authenticated lexical compact receiving (OPT915, source only)

The authenticated lexical comparison now uses the same general compact
canonical receiving twin as the scalar comparison, for both independently
constructed roots. Authentication, cold-dead handling, ordered Label/signature
algebra, weighted multiplicity, work, and scratch clearing retain their existing
order. This selects no narrower label or exit shape. All byte/argument/result,
node/vector/Rc and refusal/Drop invoices remain those of the existing compact
producer; only successful Diagnostic-bearing primitive Result carriers absent
from that producer are absent here. Its real failure still converts to the same
six-field Diagnostic at the original failure boundary. No cap, funding, refund,
profile, certificate or semantic authority changes.

The complete immediate-old authenticated caller remains independently compiled,
with its complete source image and unchanged helper dependencies bound by the
owning controls. The general canonical receiver and cold-dead path remain
unchanged. Tiny, branch and full ordinary/selected lexical callers compare
semantic outcomes and cumulative work; physical/heap observations include cold
setup and final Drop. Exact/short/zero work and backing, reserved floor, sticky
refusal, lexical target/order hostility and all six diagnostic fields are
additional unexecuted definitions. The enclosing ReplayFactoredCanonical debit
of 14,399,250 bytes in one historical selected refusing prefix is ranking
evidence only. No amount of that debit is claimed as savings, and no application
recovery or runtime qualification is established by this source batch.
