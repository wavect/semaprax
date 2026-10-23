# Expression-fact type cursor

Status: focused native cursor checks pass; full CI qualification pending.

Audience: compiler contributors and reviewers.

The function-owned type cursor grows from two slots to the unchanged 514-slot
depth limit. Each replacement reserves its complete backing before allocation,
while the previous backing remains live. Cursor reuse authenticates the same
payer's sticky state. Initialization and relocation spend the existing work
account; no byte, work or depth limit changes.

`capacity.before.txt` preserves the complete immediate predecessor.
`source-inverse.json` authenticates the entire current source and reconstructs
that exact predecessor. Both existing historical source guards retain their
complete comparisons and original hashes after this exact inverse; hostile
truncation, appended bytes and an altered growth operand must fail. The complete
old implementation remains independently compiled in `before_types.rs`.

Private larger-limit measurements showed lower actual charges, but are not
default-budget, current-head CI, or full-funding qualification. Local tests and
remaining default-budget failures must be reported separately.

The prefix source regressions retain their original complete 885 proof. Their
current-root entry APIs receive raw compiled source rather than that proof's
already-restored historical image. The A3 arena uses its existing Previous16c
input capsule. Original source inverse rows and final hashes are unchanged;
`prefix-ingress.json` and the two complete before images bind the correction.

The focused native run passed thirteen original and new cursor controls, three
arena runtime controls, six return-operand accounting controls, and both actual
frozen-pointer layout checks. The complete arena source guard also passed when
its qualified source acquisition read the actual checkout. The two prefix
source controls and their original source-composition hostile controls passed.
These results do not establish that all CI jobs pass.

All four source-audit inventories now join the complete `before_short.rs`
module alongside the original production-capacity children. The historical
current images remain available through the finite source overlay.

The accounting regressions retain complete immediate-before implementations.
Their equations now include the full output/error-slot edge ABI, the extra tree
fallback selector, and every reached transition query, callback and terminal
bank. The original receipt walk and its actual caller coordinates remain
unchanged. Original complete source
comparisons and returning-ABI guards consume their registered historical views,
with separate checks binding the current continuation.

The third original prefix inverse now selects its existing registered HIR and
custody entry views after the independent 885 proof. The arena entry authenticates
the complete cursor generation and reconstructs the complete original arena;
all three original edit lists and full final comparisons remain unchanged.
`prefix-tests.before.txt` retains the entire immediate-before test module.
The actual third source test passed in the source-only harness. These source
checks do not claim that the remaining historical receiver guards pass.

The macOS registry SDK failure has a separate setup fix: CI fetches the exact
frozen SDK dependency closure before running its offline test. The frozen lock
and renderer manifest remain byte exact; online provisioning and offline fetch
both passed locally. This does not claim a hosted CI pass.
