# Scoped diagnostic recovery v1

Status: source-authored bounded implementation and controls; execution pending.

Audience: compiler contributors and diagnostic-tool authors.

Issue #806 adapts the sound portions of #799/#800/#804. Recovery is private to
source verification. It grants no HIR, graph, cleanup, backend, storage-payment,
or program-admission authority. This changes diagnostic selection on already
refused source; codes, messages, help, paths and spans retain their original
owners and presentation schemas.

## Exact scope and precedence

`source_verify/recovery.rs` retains borrowed authored names and original
diagnostic ordinals in lazily acquired rows keyed by the existing lexical scope
ordinal. Scope entry records its parent and resets reused ordinals. A completed
initializer publishes a refused `let` only when its continuation observed a new
error or a read already bound to an original refusal. Reads before declaration
and self-initializer reads remain original errors. Ended blocks and former
sibling arms provide no facts to later scopes.

Existing binding, availability, partial-move, native-unit, escaping-owner and
function-value checks precede recovery. A missing-declaration marker never
outranks a real checked binding or valid function value. A successful binding
clears its same-scope missing marker. Closure scopes have no recovery parent;
capture admission and independent closure errors remain intact.

Annotation expression facts use exact borrowed AST identity, never a source
span as identity. The invocation-local pointer keys are not dereferenced,
serialized or iterated to choose diagnostics. Re-entry clears an old fact.
Only annotation-refused reads acquire expression entries; transparent block
lookup follows the evaluated tail chain, not statements or sibling bodies.
There is no authored-function traversal on a dependent read. Lookup visits only
the active directory ancestry. Ordinary successful scope forks acquire no
directory. Signature recovery rechecks each affected parameter once per invocation after an
existing `SPX-T001`, binds the complete authoritative refusal to the real binding
metadata, and never repeats declared-type checking per read. Overlapping forged
spans and duplicate parameter names cannot substitute another binding/type.

## Intentional changes and retained boundaries

- Refused unary and binary operators provide no fabricated checked result.
  This includes string arithmetic (`SPX-T250`), unary operand mismatches
  (`SPX-T206`), binary operand mismatches (`SPX-T208`), equality mismatches
  (`SPX-T207`) and missing operands. The complete saved operator receiver and
  recursive oracle retain their original results as independent controls;
  original primary diagnostic fields remain exact. The inference corpus's
  original `first(1,true)+identity(false)` fixture additionally pins all six
  fields of its unchanged `SPX-T208` while requiring no current checked type.
- A missing local read or alias of a completed refused initializer adds no
  repeated `SPX-T202`. Other names retain their independent errors.
- `SPX-O104` is judged only when the real body type matched the declared result,
  or native-unit refusal still requires the original checks.
- An annotation mismatch carries a separate borrowed expected-type refusal
  view. Reads retain the full actual type, mode and native-unit flag, including
  real owned and borrowed values. Calls, fields, conditions, guards, contracts
  and direct returns may omit a type symptom only when that view explains the
  specific expected type or the owning operation's admitted type predicate.
  Ownership transfers, availability, places, loan activation/release and native
  capability errors still consume the actual metadata. An unrelated expected
  type retains its error. Fixed declared call/constructor results and successful
  explicit annotations provide a fresh type obligation. Direct aliases and
  transparent block tails retain the original annotation view. Projections and
  joins requiring richer refusal derivation remain at their original physical
  type checks; no nominal field or branch result is invented.
- Unknown parameter types neutralize inference only for cleanup-inert declared
  carriers whose existing `SPX-T001` exactly identifies that parameter. Existing
  but inadmissible types and ownership-mode errors retain original behavior.
  Unknown result types use the declaration pass's own diagnostic frontier.
- Unknown record types retain their declared-type root error, duplicate fields
  and independent field-value errors, while dropping only repeated missing-type
  field symptoms. Known records retain complete field checks.
- An absent payload binding in a refused variant pattern gets a marker only in
  that arm, after all original pattern/name/binding checks. Independent missing
  coverage (`SPX-M101`), other names, siblings and closure errors survive.
  No match-wide coverage suppression is admitted.

## Storage and unchanged limits

The declaration recheck scratch and directory's Vec rows and BTreeMap nodes are real acquisitions even though
the keys borrow source. Ordinary source verification has no physical replay
constructor payer. These acquisitions are explicitly unpaid ordinary residual
storage. No phase allowance, forecast increase, verifier limit increase, paid
authority or zero-allocation error-path claim is introduced.

Existing source bounds, frame limits, and exact 320-byte verifier-frame and
312-byte variant-match-state assertions remain. Observer controls select actual
requested allocation layouts through receiving Drop; they do not infer RSS,
agent-token savings or paid capacity from borrowed key length.

The source-authored `recovery_tests/funding_boundary.rs` controls additionally
pin actual unpaid constructor/work behavior under zero, exact-observed,
one-short and already-sticky active budgets, complete receiving Drop, and
ordinary six-field diagnostic compatibility. They do not establish a paid
route. [Recovery constructor funding design v1](RECOVERY-CONSTRUCTOR-FUNDING-DESIGN-V1.md)
records the separate capacity-failure transport and acquisition owners required
before implementation. The explicitly authorized fallback stops at design
because the current bool/Option/unit and source-Diagnostic result channels cannot
faithfully propagate a payment refusal. Controls are authored, not executed.

## Full preservation receivers and qualification

The unchanged recursive oracle remains independent. Test-only
`source_verify/recovery_legacy.rs` and its enter, blocks, operators, records and
match_arms children preserve the complete receiving bodies from
`5728c9e48fe2afe5cd77e9edf000cbe171035bdb`. Only names/visibility, the additional
continuation field and inert test-only state are adapted. The saved declaration
receiver uses these bodies and the saved contract receiver; original recursive
and function-declaration bodies are retained. Source-text audit joins include
every added recovery/legacy child.

`source_verify::recovery_tests` owns the complete 60-module authored corpus from
PR #804 at `d36d47daef42e5d7e06e16a7ebade20be66ba95e`, exact old ordered code
lists, separately stated current lists, first diagnostic equality and complete
original diagnostic subsequence preservation. Its boundary child covers
scope/closure/function precedence, owned and borrowed annotation controls,
independent match coverage and actual repeated receiver acquisition/Drop.
Its refusal_dependency child adds full old/current expected-type controls,
actual ownership/place/loan state equality, fixed-result reset, identity/re-entry,
native-unit, contract, projection and success canonical/graph boundaries.
Its fixture_controls child retains each original fixture string that failed at
runtime (missing final arm comma, local named `result`, non-`i64` or
parameterized `main`, explicit `own string`, missing Boolean catch-all, missing return semicolon) as an executed
negative control with complete hand-authored six-field invariant diagnostics
(`SPX-P106`, `SPX-T104`, `SPX-O002`, `SPX-S109`, `SPX-T201`, `SPX-T257`) beside a corrected
admitted counterpart (trailing comma, `outcome`, helper plus valid `main`,
`owner: string`, Boolean catch-all and integer `match`, terminated return). Authored only; not executed.

The original `iterative_verifier_tests` corpus remains unchanged.

Required next-batch selectors: `--lib source_verify::recovery_tests`,
`--lib source_verify::iterative_verifier_tests`, source-verifier declaration and
constructor controls, `--test language variants_semantics`,
`--test language verifier_hints`, `--test source_locked_contracts`,
`--test module_size`, the Native Rust builder ledger-capacity owner,
production-library Clippy, and any actual changed diagnostic/guidance-pin
producer. Full profiles are excluded by the explicit user instruction for this work.

No Cargo, build, test or full-profile gate was executed during this implementation
batch, at the user's explicit instruction. These are authored controls, not
passing or current-head execution evidence. No hosted, measured token/cost/turn,
public-support or production-support claim is made.

Annotation refusal views are limited to immutable bindings. Mutable assignment
flow keeps its full original diagnostics until a dependency join model exists.

The ordinary public generic producer retains its exact authored template
while checking each validation-only scalar specialization. A private borrowed
origin binds that ProgramSource subject, template pointer and specialized-body
pointer. Body, requires and ensures entries pass this same origin. Receipt
recovery rechecks a missing parameter in the template's real
generic environment while retaining the actual specialized binding metadata;
foreign subjects, stale bodies and all six-field receipt mismatches fail closed.
The origin adds no function clone, owned backing or admission authority.
A generic parameter profile does not issue a dependent shape refusal when the
ordinary declared-type checker identifies an unresolved root identity. A known
outer type with a missing nested component retains its independent profile
refusal; known sibling profile errors remain independent. Declaration-recheck scratch, diagnostics and
recovery directory storage remain ordinary unpaid verifier acquisitions; this
change does not create physical funding or alter sticky budget receipts.

The specialized signature receiver pairs each authored parameter with the exact
current parameter held by the producer origin. Binding equality uses the current
type and mode; receipt rechecking uses the authored type and generic environment.
This includes unresolved roots whose arguments contain substituted parameters.
The same origin authenticates an unresolved root return through full diagnostic
equality. Only the actual postcondition return carrier exposes that dependency
through `result`; preconditions and bodies gain no result binding. Known outer
return carriers retain independent profile failures. These paths add ordinary
unpaid error scratch and directory storage, with no funding or layout claim.
