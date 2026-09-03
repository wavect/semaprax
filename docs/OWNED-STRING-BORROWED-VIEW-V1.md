# Owned String Borrowed View v1

Status: **Partial; library implementation and regressions authored, unrun.**

Audience: language users, compiler contributors, and ownership reviewers.

`string_as_str(value)` is the compiler-owned `core.string.as-str` operation. It
accepts exactly one unprojected named owning `string` place and produces a
non-escaping `borrow str` view. The operation does not transfer, clone, drop, or
settle the owner. The owner remains the sole cleanup root.

Resolution emits `BorrowPlace` with the exact operation identity and owner
`ValueId`. Source verification and checked-HIR validation reject literals,
ordinary call results, projections, moved roots, wrong storage types, and forged
non-view operation identities. Ordinary loan tracking binds a local view to the
same owner root and prevents overlapping moves for its lifetime. Canonical
source uses the reserved spelling `string_as_str`; the semantic graph exposes
`core.string.as-str` and the rooted borrow relationship.

This view is not the only authenticated root of an immutable borrowed-`str`
local. `arg_utf8(index)`, owned by [Bounded Language Command
I/O v1](BOUNDED-LANGUAGE-COMMAND-IO-V1.md), roots such a local on the single
invocation-owned argument arena, which is not a named in-scope owner and
therefore mints no loan: repeated or dynamic argument reads neither mint nor
recharge roots. Checked-HIR validation authenticates exactly the three
`borrow str` producers the resolver admits — a `borrow str` parameter, this
owning-String view, and the command-argument view — and rejects every other
value shape.

Native lowering forms the existing length-aware `spx_str_v1` carrier over the
owner allocation. Frozen terminated-string profiles use their existing
terminated representation; length-delimited profiles use the authenticated
stored length. Wasm lowering retains the existing scalar carrier and validates
it through the admitted arena boundary where that boundary exists. The opaque
standalone internal-String profile does not yet admit this view because it has
no authenticated handle-to-borrowed-carrier conversion import. Neither backend
transfers or clears the owning carrier while forming the view.

The reference interpreter stores the exact logical owner identity and immutable
UTF-8 contents in its borrowed value. Its internal `Arc` materialization is an
abstract evaluator representation. It is not evidence of physical borrowing,
allocation equivalence, cost, performance, or runtime memory layout.

`change_function_signature` may use `borrow_str_from_owner` together with the
existing Bytes mapping for a bounded total of one through eight distinct owners.
Each String owner must have exactly one authenticated, unprojected, body-local
`string_as_str(owner)` use and no contract or other owner use. Every caller
stages original arguments once from left to right, derives views afterward in
mapping order, and retains ordinary caller cleanup. Full Project replay remains
the authority for loan lifetime, cleanup, profile, package-conflict, and target
admission.

This stage grants no escaping view, mutable view, projected String view, borrow
of a temporary, borrow `Bytes`, external ABI compatibility, automatic package
or consumer migration, network/filesystem/process authority, runtime execution,
physical allocation claim, or hosted evidence.
