# Private path-boundary / borrowed-chain reconciliation

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base: `2c4306e1b19c9899a4b11abe7ca1e604f91fde15`.
Original component: `755c082541ffc8b724c7b932428b6bb80153ee6b`.
Rebased code: `0ca588936f00d2d45b13d35510721a13332015cf`.

This carries the original 755 implementation and all six new Rust owners onto
current Main-f1, compile-correction, and audit-pieces ancestry. The 15 changed
Rust inputs are exactly the original component bytes, except the already-required
`ExpressionId` type annotation from bc6 is retained in lexical_boundary/paid.rs.
The current selected_context child visibility and formatting remain exact base
bytes. CHANGELOG and ARCHITECTURE retain both histories; only the original two
appended lines in each are added to the base.

`binding.json` authenticates four complete raw current/component pairs: both
history documents, the corrected macro source, and the existing child visibility
source. The unchanged original verifier is replayed against that finite,
authenticated component domain using temporary read interception; physical source
is never rewritten. This preserves its original 12 whole-input inverses, complete
old path receiver, complete original Diagnostic algorithm, 11 labels, six new
owner hashes, caps, and three selector checks. The original packet is unchanged.
No historical Rust execution is attributed to these current inputs.

Run the bounded source check:

```sh
python3 docs/evidence/path-boundary-755-reconciliation-2c430-20261016/verify-source.py --repo .
```

Actual outcome: PASS_SOURCE_ONLY. No Cargo, Rust tests, Clippy, captures, broad
generators, or current integration writes were performed. All three exact lib
selectors in `pending-selectors.json` remain pending. The new real per-call
receiving payments can worsen common-case costs; no common-path, application-fit,
allocation, or reservation improvement is claimed. Preserve these tradeoff owners
in any eventual focused qualification.

The future canonical composition still owns the whole raw-source front, all
three audit unions, and current authority manifests. Each of the six new files
must remain in those complete inventories. This private component does not alter
the currently frozen producer scope. Raw-inventory prototype work remains in its
separate original SSD checkout and is not part of this commit. Issue #781 stays
open for remaining families and actual qualification.
