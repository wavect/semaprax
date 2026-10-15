# Paid endpoint ordered union, source packet

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Basis: be8ff06ff1e3319c63a3e00f82f7085dd7b83ce9. Source-only; no Cargo,
Rust execution, allocator measurement or application-fit claim in this packet.

Both private paid planner callers construct edge rows through
paid_live::edge_liveness: increasing loan_index appends LoanId to each edge.
Endpoint unions now borrow those ordered rows in a paid slice heap, count the
unique output, then repeat the heap merge into exact paid output capacity.
Each pass is O(total row IDs log(nonempty degree)); no repeated accumulated
union copy, per-ID tree allocation, sorting, new quota or logical-work credit.
Empty unions allocate neither heap nor result backing. Malformed unordered
internal rows retain the prior paid ordered-set union behavior; input rows and
canonical cleanup vectors are never rewritten. Full prior materialize source
is compiled independently in the before child. Existing old planners remain.

New fixed directory-node and vector-backing labels apply only while the exact
active stage is the outer CanonicalLoanStorage. Known candidate/CFG/append
phases win. The labels add no account, budget field, trace, string or debit.
Existing enum/Budget/receipt/guard layout controls remain selected. These labels
may classify a future refusal; no present 64-byte refusal is attributed to any
specific node or vector, and no savings/fit amount is inferred from it.

Three full receiver/union controls and two fallback-stage controls are authored
in pending-selectors.json. Whole-plan comparison includes every LoanPlan field;
missing termination preserves the full old error. Physical controls retain
whole constructor/result Drop observation and requested backing <= same-owner
debit, 0/short/exact/plus-one and no allocation after sticky refusal. The retry
uses an iterator that panics if read, proving first-refusal authentication comes
before input traversal. Existing work limits/canonical output order are intact.

All original source bytes are retained in exact-source-inverses.json archives;
new-children.json binds the three child bodies. The native full-source inventory
joins all three beside their original owner. Other historical packets require
additive current-to-basis inverse composition after root integration; no old
archive/hook may be replaced.
