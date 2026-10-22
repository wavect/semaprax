# #854 acceptance controls (unexecuted)

Base: `2c4306e1b19c9899a4b11abe7ca1e604f91fde15`.

The literal issue requires independently constructed equivalent roots; complete
old/current dictionaries and IDs; real reservation/allocation/live/peak data;
signature hits/misses and dictionary node counts; growth cuts, sticky and foreign
ownership failures. Earlier owners already retained the complete old algorithms,
all-unique wide inputs, reservation/heap/Drop measurements and zero/half/one-short/
exact total growth cuts. They did not yet substantiate the additional scenarios
added here. No prior PASS is attributed to these additions.

Four existing files change. The two existing arena_tests owners and existing
arena_source custody owner retain their selector names. No new module, top-level
binary, public API, constructor layout, work debit, fee, cap, algorithm, backend,
or shipping dictionary operation is added. In canonical_shared.rs and complete
before_arena.rs, the only additions are cfg(test) calls in the actual Some/None
signature-query branches. Both calls observe the existing query exactly once;
they do not perform a second search or synthesize expected hits. Const TLS Cell
counters allocate no backing and retain no owner or payer. Removing these exact
hooks reconstructs both complete base files byte-for-byte.

The semantic owner now builds old/current automata independently for its original
work/overflow/cycle cases, and separately for each complete duplicate/unique/tiny
corpus. It compares every returned ID, full signature and state dictionaries,
and remaining work. The physical owner constructs a separate graph outside each
old/current observation window, retains complete budget+owner Drop inside it,
and reports actual hit/miss counts and both dictionary node counts along with
existing Metrics (allocations, freed, live, peak). It checks equal actual counts,
misses=signature nodes, lookups=state nodes, real duplicate hits, and zero hits in
the all-unique-wide case. Original favorable duplicate reservation assertion and
unfavorable unique/tiny reporting remain; no measured gain is claimed yet.

The semantic owner also calls the same rejection probe against both complete
receivers after a real completed insertion. Probes cover a required active-ledger
refusal, pointer-distinct function, distinct ReplayBudget, and nested foreign
active ledger followed by restoration. Both real rejected inputs are prepaid
before the hostile condition. Calls compare all Diagnostic fields, preserve
work, dictionaries, lookup counts and active remaining bytes, and do not allocate
inside rejection. The nested TLS budget's own ordinary allocation is distinguished
from the allocation-free rejection call. Retry under restored original authority
must remain rejected. A real prior refusal is checked for exact requested/
remaining/floor/stage fields and presence of its source site after all retries.
Complete receiver, budget, inputs and temporary errors drop inside measure.

`source-inverses.json` contains all four complete Git before archives and exact
whole-input forward/reverse edits. `verify-source.py` is finite source custody
only and passes without executing Rust. Targeted formatting checks and git diff
whitespace checks are source-only. Pending three exact lib tuples are in
`pending-selectors.json`.

Future canonical integration MUST prepend the current-to-component/base source
front to BOTH inherited arena854_input and arena854_previous source routes (and
the ordinary global front), preserving all old tables. The existing source owner
now restores before_arena through arena854_input before its unchanged full-body
contains assertions; its original four-owner SHA checks also remain. In this
isolated component those inherited SHA gates require that later global join.
Regenerate current authority manifests only in the parent safe window. Existing
raw unions already contain all four owners; no new file is introduced.

No Cargo, tests, Clippy, captures, benchmarks, broad authority generation, ROOT
writes, or #854 closure are claimed. Unchanged Normalizer/codec/application gates
and actual pin consumers remain the parent's literal-issue acceptance evidence.
