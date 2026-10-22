# Lexical canonical successors v1

Status: private source contract; executable qualification is pending.

Audience: compiler contributors and reviewers.

This source-only #884 candidate retains primitive receiving frames in the actual
ordinary canonical traversal. Compilation, focused execution, application fit,
and measured savings are pending. It changes neither language semantics nor
capabilities, admitted budgets, work quotas, or retry profiles.

## Owner and admission

`canonical_shared::lexical_successors::frame_loop` implements the actual
`Canonicalizer::id_inner_into` method. Its unit and general macro expansions
physically contain the old successor and one-pass traversal bodies. Only the
existing ActiveBuilder payer enters them. Selected, Oracle, and Legacy routes
retain their original helper. No runtime pricing function, callback, forwarding
method, or second payment is introduced.

The existing `id_with_receiving` validates the held ClosureReceiving and Memo
before the loop or resident flags are used. Each real frame-loop invocation
resets both flags. Thus HIR and PLAN are separate activations; a cached result,
function identity, or flag cannot authorize a foreign budget or ledger. Flags
are resident fields in the already-paid full Canonicalizer initializer and
receiver. They become true only after the complete original-frontier request
succeeds, and never choose an account or convey authority.

## Typed receiving and atomic frontier

Let U be twice the actual/formal tuple
`(&mut ReplayBudget, &ResolvedFunction, usize, &str)` plus twice
`Result<(), Diagnostic>`. Let V be the corresponding node tuple without usize
and its two Result slots. The retained primitive bank is P = 24U + 5V for EACH
of the two distinct lexical expansions. The typed ledger lists every site.
Only the established narrow primitive-frame criterion applies: fixed function
and budget places, nonrecursive sequential use, ZST success, one terminal owned
Diagnostic, and no escaped or overlapping receiver. Both owned weighted-finish
helper transports are excluded from P. Fresh Option, tuple, enum, iterator,
label, target, vector, Rc, and backing formation remains paid at every actual
old lexical position, including the dynamic PaidMany calls.

D is the complete unchanged old `one_pass_receiving` typed expression, including
its original helper transport inventory. R is four bool slots plus two usize
slots per use for resident flag tests/stores and the selected scalar operand.
A is the actual constructor-control difference due to the real field layout;
no zero layout change is assumed. A full Self payment precedes flag formation,
and the existing original reserve operand frame receives the selected usize.
The extra R is charged on every expansion; it is not retroactive authority for
an otherwise unpriced helper or transient aggregate.

One original-frontier reservation requests D+R when cold or D-P+R when warm.
These are compile-time constants selected by the resident place, without new
runtime arithmetic or pricing-call transports. Admission is atomic. Failure
leaves the flag unchanged and retains the original first-refusal/sticky rules;
its six fields match the real reserve primitive at the actual request.
A short new frontier may differ in requested bytes from the old D frontier;
this is disclosed and exercised, not projected into an old raw diagnostic.

For N actual one-pass entries and M cold expansion banks, the conservative paid
reduction is (N-M)P-A-NR. A single id_inner activation has M<=2; one complete
HIR/PLAN comparison has M<=4. No removed outer helper/compound Result is credited.
N and M must come from actual current diagnostic counters. Small inputs may pay
more; the original full algorithm remains the measured baseline. No gain,
application-fit, or two-attempt recovery is inferred from this formula.

## Semantics, history, and owning controls

The same left-to-right closure walk, subset request of moves.len()+1 per node,
weighted addition, checked overflow, real distinct-child promotion observation,
ordered label publication, frame push/scheduling, and teardown remain. The
macro uses the already-held refusal destination rather than constructing an
intermediate helper Err. The selected first diagnostic and all owning Drop
remain real. Complete states/signatures and ordinary/skeleton work are compared
against a separately compiled complete immediate a6c Canonicalizer, including
its own terminal-leaf impl and unchanged helper/pricing dependencies. A complete
old production comparison caller exercises both real HIR and PLAN roots.

The fixed diagnostic census has 12 old/current x other/HIR/PLAN x general/unit
cells with checked counters, real function/budget addresses, and a diagnostic
invocation serial. It does not allocate or grant authority. Overflow invalidates
the report. Synthetic routing checks are not actual reservation evidence.
Actual observers are explicitly installed by the new owning controls; an
application without such an observer has no new N/M evidence.

All original tests/caps and whole source snapshots remain. New owners cover
complete graph outputs, repeated and distinct targets, equal Observation values
in distinct Rc owners, overflow/cycles, real work cuts, cold/warm unit/general
exact/short/zero/floor frontiers, independent full-six-field primitive oracles,
empty sticky retries, foreign/restored scopes, and complete allocator/Drop.
The existing exact-two-attempt owner at CORE300 index11 remains an independent
required application target. No fixture, cap, profile order, or assertion is
changed to make it pass. Later qualification must report actual paid, allocated,
peak, work and failed-prefix outcomes, including any regressions.

## Source custody and qualification boundary

`docs/evidence/opt884-lexical-successor-owner-20261017` retains whole immediate
root/helper/caller/terminal sources, finite full-byte inverses and typed ledger.
The old borrowed-label helper is byte-exact. Six new Rust modules must enter all
owning raw inventories and finite source acquisitions at a later combined
source freeze; this candidate deliberately does not run or retarget binders.
Scoped rustfmt preserves literal contents and keeps all modules <=1500 lines.
The source checkpoint alone does not implement the completion matrix's gate.

## Outer application diagnostic successor

A separately retained source successor adds test-only `SuccessorAppCensus`
scopes as the first local in the existing codec808 preparation and catalog
normalizer owners, before fixture/source/ledger construction. Their Drop emits
fixed counters only after later locals and unwind cleanup. The codec owner
continues to resume its original panic; no status or refusal is replaced.

The outer scope is independent of the original hostile single-function/budget
observer. Actual current and complete-before `id_with_receiving` entry guards
bind function address, budget address, diagnostic serial and current HIR/PLAN
phase. This records identity, not authority: all real Memo/ClosureReceiving
checks and their order remain unchanged. The guard resets invocation state and
settles pending bank refusals before a later pair can enter. Fixed aggregate
cells distinguish attempted/acquired entries, attempted/acquired primitive
banks, requested/acquired bytes and refused entries. HIR, PLAN and other cells
remain separate. A refused cold request is not an acquired M bank.

The new copy-only TLS state never reads a ledger, allocates an event buffer,
spends replay work, changes payer selection or enters production compilation.
Nested observation, foreign/missing identity, unexpected pending ownership,
serial overflow and checked counter overflow mark the report incomplete/lost.
Such a report cannot establish savings. A complete failed-prefix report is
still a failed-prefix measurement, not full-path N or fit. The successful-path
(N-M)P-A-NR equation must not be applied to failed attempted entries as if all
banks were acquired. Actual application execution remains pending.

Application reporting restores the fixed counter state before best-effort writes
to locked stderr. Closed diagnostic streams and unavailable TLS cannot panic
through the new application scope/invocation cleanup or replace an original
owner unwind. The source-only closed-writer control covers normal return and
fixture teardown during an unchanged synthetic panic; it does not intercept an
actual application producer. Missing or truncated report output grants no
measurement or qualification claim. Execution remains pending.
