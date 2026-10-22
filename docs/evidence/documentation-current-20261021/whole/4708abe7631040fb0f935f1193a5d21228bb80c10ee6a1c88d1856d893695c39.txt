# OPT831 lexical scratch and cold activation receiving

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only component based on `364d824194cd1df76b5c273aa0fb9cbafb602e29`.
The actual strict owner on that subject reported old10,829/current11,566 at width0,
allocations12/11 and allocated bytes1,517/1,389. It remains failed. This component
has not been compiled or executed and claims no numerical recovery or closure.

Two shipping files change. There are no new modules, owners, fields, allowances,
refunds, caps or test bodies. All original strict/unit/bitmap/held test assertions,
ordinary Memo/RetainedMemo and complete compiled before_unit receiver remain exact.

* `prepare_scratch` disappears. Its first-use block is lexical at the same id and
  cold-query positions. Each caller stops constructing that helper's
  `Result<(),Diagnostic>` and stops forwarding its self/function/budget arguments.
  Only the explicit corresponding Result receiving term is removed at each call.
  All three retained vector headers, scratch flag, actual scratch backing, work,
  nested reservation Results and scratch-paid publication order remain.
* `activate_closure!` expands the original unbound cold Memo activation at its
  existing caller. Preflight has already established unbound/nonfailed state.
  Zero authentication still precedes full Memo and Option<Binding> payment and
  binding capture. The actual inner authenticate Result and its explicit
  authorization-receipt payment disappear. Every remaining reservation Result,
  outer id Result, Binding initializer/capture and full storage charge survives.
  The narrow Binding field visibility permits this parent lexical expansion; its
  complete original authenticate function remains exact for other adapters.
* Production cold errors move directly into the already-paid invocation refusal
  slot and abort only the activation block. The test id still activates BEFORE
  acquiring its invocation slot, keeping the original initial unreceived failure
  boundary and returning the same outer Result. Foreign/stale preflight occurs
  before payer queries; sticky failure remains unchanged. No initial failure ABI
  repair or output-funding claim is made.

`verify-source.py --repo <checkout>` checks both complete current SHA→unique edits
→Git364d/archive inputs, original authenticate-body identity, precise production
scope, unchanged original assertions/independent receivers and source-size limits.
`source-proof.json` is source-only, not executable qualification.

Protected new runtime input: `current-to-364d.json`. The compositor must first
route independent raw-final inputs into this complete component domain and apply
these two strict rows before inherited364d projection tables. It must preserve all
old runtime JSONs byte-exact. This component intentionally does not edit global
source helpers/audit inventories; canonical composition owns that front. There
are no new child includes. Existing strict and three unit selectors remain in
`pending-selectors.json`; their old bodies are unchanged.
