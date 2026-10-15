# Two actual a231 controls corrected; no production algorithm change

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual owner118 failed in the complete old materializer before.rs74 because the
zero-loan public plan had taken its early return and carried no CFG edge rows.
The control constructed a nonempty CFG anyway, violating the internal
materializer's edge_live.len()==cfg.edges.len() precondition. The fixture now
supplies one empty liveness row per actual edge, retaining all zero/one/three
loan complete comparisons and original work/error assertions. Nonempty input
rows are bound to the exact complete canonical edge tuple order.

The original malformed zero-row input remains an explicit inherited old/current
panic control with equal index-panic payload. With real loans, missing
termination plus malformed edge rows must select the full stable old H006
termination diagnostic before any bad row access. Both complete materializer
algorithms and every actual positive/physical/control remain unchanged.

Actual owner131's complete caller source differed from its unchanged archive by
only the five-line LoanCanonicalAppend stage wrapper. One exact unique inverse
now removes that named wrapper before every existing hook. It does not normalize
source or alter the complete original/hash. A source-only mirror applies the
same ordered projection and proves full original byte equality. #829's source
owner still requires runtime re-execution; this packet does not claim it passed.

Two full original test files/inverses and the actual failed log hashes are
retained. The live union child body binding is refreshed. No Cargo/compiler/
Rust test execution or root edits were performed. Re-run exact owners118/131,
all union physical controls and #829 base owners in the next combined batch.
