# Status fact borrow reuse v1

Status: draft specification; qualification remains as recorded below.

Audience: compiler contributors, tool authors, and reviewers.

The ordinary retained status receiver inserts the full status identity first, then performs the same existence lookup, then validates the unchanged lane/producer and call callee. A propagated call uses the actual borrowed payload returned by that first lookup. The immutable concrete Facts arena and source remain borrowed through the immediate producer match. No reference escapes, no verdict is cached, no source identity is inferred from an ID, and no authority is inferred from a diagnostic.

Previously the call branch immediately repeated the same paid lookup without intervening mutation. Removing that second traversal removes only its actual comparison, proof-control, and return-control operations. All lookup, AVL, and replay unit prices and caps remain unchanged. The new receiving reference to Option<CallFact> is paid in full before initialization, together with one assignment unit per source. The call arm separately prepays its simultaneously live CallFact reference and one assignment unit. No removed result/header is credited against either layout. Non-call sources honestly incur the first receiving control even though they have no repeated lookup to remove.

Duplicate identity remains before unknown-expression detection; unknown expression remains before producer/callee checks. The tree route and arithmetic/allocation/contract checks remain unchanged. A new explicit control can change where a genuinely tight physical/work budget first refuses. Complete prior status and block receivers remain independently compiled; their old exact receipts remain independently owned. Current boundaries are derived from the actual complete route. Same-budget retry cannot reopen a refused replay.

Owning controls compare complete old/current results and all diagnostic fields, actual full receiver allocation/Drop, independently priced removed lookup costs, all new receiving payments, and exact/one-short/zero work and bytes. The old retained-control exact test executes its complete old producer through one explicit lexical adapter, preserving every original assertion. Source custody joins current/old/tests and reconstructs all protected originals. See docs/evidence/status-fact-borrow-reuse. Execution and whole owner17 recovery remain pending.
