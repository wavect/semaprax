# Invocation-paid private-authority roots backing (#765)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only implementation from `787f84774e935833cd0d8de17c6c8da21d7b4986`.
No Cargo, Rust tests, builds, Clippy, application execution or measured savings
are claimed. `pending-selectors.json` records six new owning selectors and the
three unchanged owning Invocation tests for the next consolidated qualification.

The current `ScratchInvocation` retains one full-census roots Vec. Its exclusive
lease holds both the predecessor state and roots RefMut guards through every
use in all four complete declaration/signature consumers. The guard clears
logical roots and DFS frames on return, semantic error and unwind. Incomplete
preparation poisons the original state. Reentrant acquisition cannot obtain a
second mutable borrow or expose partial roots; failed overlapping acquisition
leaves the first lease intact. Roots, remaining WORK, private predicates,
duplicate comparisons and current-HIR callback answers are regenerated for each
caller. Exact Program-slice, authored-map, caller pointer and captured active
ledger identity are checked at each reuse boundary, including empty leases.
Signature/contains queries check that ledger again before lending a view.

The first actual full-census roots acquisition follows the unchanged DFS
acquisition frontier. Its checked Layout and exact capacity are admitted before
allocation; excess actual capacity is settled and refused. Later leases remove
only replacement roots backing that is no longer allocated. Nothing refunds,
resets or increases any budget, floor, WORK/depth/leaf bound or forecast. No
source, identity, HIR signature or successful callback verdict is cached.

`new` prepays the complete owner, Option/Result receivers, predecessor owner
constructor receiver, ledger identity and parameter carriers before formation.
`lease_controls` names the actual guard, RefMut locals, query-view/Result,
prepare/finish/callback arguments and receivers, Layout and capacity controls.
The common `Current` enum/Result/parameter receiving is charged on both leased
and ordinary fallback routes. A denied bootstrap returns None without allocating
failure-report text; the existing outer workspace diagnostic boundary translates
that failure. Existing traversal/semantic Diagnostic producers remain unchanged.
There is no flat allowance or inferred old charge credit.

This can cost more for empty/single-use/ordinary fallback cases. Retained backing
also overlaps later HIR memory. The new controls explicitly preserve these
unfavorable cases. Actual physical tests independently execute the full old
sixteen-acquisition constructor and current receiver, inspect cumulative paid
bytes, actual allocations/reallocations/frees/highwater/live backing and retained
capacity, and check exact, one-short, floor, zero, sticky and Drop behavior.
Other controls compare root order and pointer subjects, exact remaining WORK,
current callback counts and complete Diagnostic equality, including all four
complete consumers, malformed same-size proof inputs, foreign slices/maps/callers
and ledgers, overlapping leases, partial DFS, layout overflow and unwind cleanup.
These are authored assertions, not execution evidence.

The complete original Invocation and its original tests remain unchanged apart
from test-only method attributes and the new child registration. The original
PrivateTypes, root walk, proof, helper predicates and caps remain byte-exact.
Complete predecessor compact/signature consumers and full declaration consumer
are independently compiled in `validation/before_roots.rs` and
`before_private_roots.rs`. Only their explicit relocation paths/visibility differ.
Three existing prior core modules change imports to those receivers; their full
function bodies remain byte-exact, so old and current arms do not accidentally
share the new constructor or lease implementation.

`before/` preserves all 13 changed preexisting files and seven unchanged owning
inputs from exact Git. `source-inverses.json` proves each complete after-to-before
inverse. All five new Rust modules and the three rebound original core callers are
raw-bound in each of the three existing full authority inventories. Changed root/caller bytes are also mandatory exact
registration-chain source bindings (`source-parent-bindings.json`); full workspace
roots retain their existing scope rather than becoming new HIR-purity inputs.
Removable literal fragments preserve the
complete old guards and their line counts. The native inventory already had
1530 lines at this base, exceeding the default 1500 rule; this lane adds no lines
there. The parent module-size correction must resolve that existing issue on the
joined source without dropping original inventory members.

Read-only source proof:

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 \
  python3 docs/evidence/private-authority-roots-scratch-765-20261016/verify-source.py \
  --repo /Volumes/Semaprax/worktrees/opt765-roots-scratch-20261016/AI-Lang
```

After integration, preserve this complete historical packet and all original
custody tables/predicates. Bind every new production/test module listed in
`current-source-modules.json` and the changed callers/old-receiver imports into
the actual final source-authority inventories and exact compilation-registration
source chains. Preserve the original ownership domains of purity predicates. Refresh independent Main043
endpoints and then the existing Main44/a3/e47 current endpoints once on the final
joined source, using their unchanged original scopes. Do not manufacture a
production-equivalence claim for this actual production change, or reuse a
prior Clippy/application receipt. Parent-owned qualification must execute these
selectors plus affected ordinary/compact workspace graph/HIR/image/cache and
application owners on exact registered source/binaries. Integration changes to
these exact files require explicit later source bindings; do not rewrite this
packet to make its fixed endpoint hashes fit a different joined source.
