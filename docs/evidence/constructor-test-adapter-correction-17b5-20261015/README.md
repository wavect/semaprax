# Test-only compiler adapter correction against immutable17b

Addresses actual17b production-PASS129s / unit-test-compilation-FAIL8 errors without changing production module visibility, existing helper bodies, carrier layouts, or observed allocation budgets. No Cargo, compiler, tests, review, Claude or ROOT writes were performed during preparation.

The two private-module imports become calls through cfg(test)-only aliases on the already workspace-visible `borrowed_imports` module. Its new cfg(test) wrapper delegates directly to the complete original inline import helper with unchanged arguments/result and performs no allocation, reservation, conversion or control decision. Production builds exclude all additions. The expected_projection root already has1670 lines and is untouched. Existing full borrowed_imports source audit joins automatically retain this test bridge.

DeclSlices itself supplies no PartialEq/Debug. Its existing borrowed `.iter()` yields complete declarations in exact order; `.eq()` compares every full Eq item and both iterator lengths, including generated suffix order. Thus both assertions retain full TypeDeclaration/Function equality without collecting/cloning or allocating. Existing error, physical, whole-Drop and allocation assertions stay intact. No production derives were added.

Every changed source has complete immutable17b bytes and a full-source inverse. Parent must preserve actual failed17b receipts and compose both current source bindings after applying the patch, then use the same136 selected owning tests; this source-only packet makes no execution claim.


Integrated only after the actual17b run terminated: exit101, 358.699 seconds, production PASS129 seconds, eight unit-test compiler errors, zero tests executed. The complete failed round is preserved in `../failed-build-17b5-20261015/` with hashes in `failed-build-preservation.json`.

One source-only custody refresh verifies all prior55packets/566rows plus this two-source correction: 56packets/568rows and all36 complete original reconstructions. Old proofs, original archives, dispatcher hooks and existing audits are preserved. Static source proof retains91 private-layout sources, exact6246 Rust file size checks, mirror3 and all4 typed projection joins. Typed selector source hashes are refreshed with their prior bindings preserved, and the complete previous selector packet is archived here. All136 selection identities remain unchanged. No Cargo, compiler, runtime, reviews or Claude ran during this integration; queued95/d7/5ca packets were excluded.
