# Ordinary DAG incoming ownership reachability (#819)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This source-only audit is bound to `cb55986d65e4707c203a950d38e76d55575927c6`.
No Cargo, compiler or test binary was executed. No production or test source,
original receiver, diagnostic, limit or numeric assertion changed. The adjacent
JSON records the complete `src/**/*.rs` DAG-reference census and exact hashes.
This is a reviewed source induction, not a compiler-generated reachability proof.

## Production construction and refusal

The two production node publication sites are
`src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs:51` and
`src/cleanup_plan/replay/skeleton_work/dag_storage.rs:118`. Ordinary construction
reserves node backing and its immutable structural pending bound before Rc
publication. Nonleaf nodes retain the acquired scratch Rc; leaf nodes have no
child edge and require no pending storage. Selected construction retains its
existing same-session edge registration and backing acquisition. Its ordinary
fallback calls `ordinary_dag`; this does not activate the selected oracle hook.

Initial `Observations::default()` and `From<Vec<Observation>>` have no prefix.
Cloning, singleton merges, extending flat tails and memo lookup preserve existing
Rc owners; they cannot introduce an unowned DAG. Ordinary freeze, union, owned
extension and prepend create their nodes through `ordinary_dag`. Selected freeze,
union, owned extension and prepend create their nodes through `charged_dag`.
`share_prefix_tail` also uses `charged_dag`. Thus by induction every nonleaf
production-created node owns its prepaid destruction scratch.

The incoming kind owns its child Rcs before the first admission check. A refused
work debit, ordinal overflow, metadata debit, bound fold, scratch acquisition or
node backing debit drops those Rcs. Each last child owner retains its own paid
frontier, even when the new parent was never published or the current payer is
sticky. No new backing is acquired during that child's destruction. Retained
external owners keep that same frontier alive through their later Drop. A child
from another ordinary work/ledger remains valid: ownership follows the child,
not a new parent session policy. Successful parent destruction uses its fully
paid aggregate bound, drains children in the unchanged reverse-pop order and
releases its scratch borrow before the incoming frame can unwind.

This argument covers DAG destruction storage. It does not assert that unrelated
Diagnostic rendering, initial index construction or every replay allocation is
funded, nor that allocator-event order is identical between old and new member
carriers. No proof object performs a physical language-resource finalizer.

## Legacy constructor census

The original allocating destructor and all legacy controls remain unchanged.
The complete saved receiver still compiles from `saved_receiver.rs`, whose first
14,172 bytes equal `saved.rs`; its two lexical access bridges remain the sole
suffix. Original source inverses continue to audit that compiled receiver.

| Legacy construction owner | Reachability |
| --- | --- |
| `Observations::insert_front`, skeleton_work.rs:479–492 | Its only production caller, `insert_front_observation`, calls it at line 358 only when `prefix.is_none()`. The helper's `None` arm inserts a flat item. Its legacy head/Cat allocation is in the unreachable `Some` arm. A present prefix goes through ordinary/selected paid prepend instead. |
| `Observations::extend`, skeleton_work.rs:495–508 | Production `extend_owned_observations` calls it at line 305 only after the selected branch has returned whenever either operand has a prefix. With the suffix prefix absent, the helper returns after flat tail extension, before its legacy constructor. The analogous line 267 caller belongs to `#[cfg(test)] extend_observations` and has the same guard. |
| `Observations::freeze`, skeleton_work.rs:511–518 | The only source callers are the legacy `extend` branch above and test-only `union`; neither creates a legacy node in production. |
| `Observations::union`, skeleton_work.rs:521–539 | Explicitly `#[cfg(test)]`; production merges select `ordinary_union` or `union_observations`. |
| `ordinary_dag/saved.rs` and `saved_receiver.rs` | Full original independent test receiver. The compiled module is guarded by `#[cfg(test)]`. |
| `ordinary_dag/frontier_tests.rs`, `factored/borrowed_canonical/tests.rs`, `factored/dag_memo/tests.rs`, `lexical_skeleton/tests.rs` | Explicit test modules manufacturing legacy inputs, including deliberately unowned and forged-bound controls. |

The textual census also includes definitions and `impl` headers; these are not
additional publication sites. Graft's name-only caller edges for `extend` also
include unrelated Vec methods. The source checks distinguish their receivers;
those approximate edges are not treated as authoritative type resolution.

## Actual evidence and pending qualification

The prior exact-head 74d903 round actually passed all ten ordinary frontier
owners, each with one body, zero failures and zero ignored bodies. The JSON binds
its immutable result file, executable SHA and individual log SHA values. All
nineteen DAG-reference/receiver source files are
byte-identical between that historical head and this source subject. Historical
execution is still historical execution; it is not a current-head receipt.

In particular, the original positive-allocation owner
`successful_paid_parent_drains_legacy_children_but_zero_refusal_preserves_legacy_boundary`
continues to assert `observed.allocations > 0` for a manufactured unowned legacy
input refused with zero additional allowance. This proves the compatibility
boundary remains visible. It does not identify an unfunded reachable production
constructor. Zero allowance cannot acquire new destruction backing, and this
audit neither rewrites that input as paid nor removes its coverage.

The existing paid production controls include complete work lifetime Drop for
empty alternatives, exact/short/zero work and physical allowances; depth
0/1/17/1024 duplicate DAG construction with every failed publication releasing
paid children; nonempty tail and Weak-header release order against the complete
original; Cat/Alt tails and external sharing; foreign ordinary ledgers and
selected conservative bounds; overflow/sticky refusal; and whole original-source
inverses. Their exact historical selectors and logs are in the JSON. All sixteen selectors in the original issue packet are included and passed
in that historical round; their current source owners remain byte-identical. Full
factored/enumerated semantic, pending call-commit/failure and current frozen-head
qualification remain the root runner's owning gates. No issue closure, full
funding, application fit, savings or support claim follows from this audit alone.

A future direct nonflat use of the legacy helpers or injection of unowned legacy
nodes into production would invalidate this induction and require payment at
that actual construction boundary before publication. The current source has
no such call. Consequently this audit proposes no production repair merely to
make a manufactured zero-budget compatibility control appear funded.

Tool-free Claude authoring identified the impossible post-refusal funding boundary.
Independent review found no concrete reachable-production defect and recommended
no source or cfg split. Its three evidence follow-ups are recorded in the JSON:
full caller/alias census, the legacy `Self` initializer, and byte-identity of all
nineteen DAG-reference source files. Both processes stayed below 512 MiB; their
exact external result/process receipt hashes are retained.
