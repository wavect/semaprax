# Actual frozen7800 compiler correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The archived build terminated normally with exit101: seven of eight build phases passed; the final core unit-library compile produced three Rust name/privacy errors. Zero test bodies ran. This was not a RAM interruption. The complete actual failed round is retained at ../failed-build-7800-20261015, including its original expected counts and drivers; it is historical failed evidence, not a corrected-source receipt.

Only three compile repairs are made in two Rust receiver files:

- before_held imports its existing sibling tests module, making the existing tests::note_actual_entry call resolve without changing that call or any body.
- Its old Canonicalizer.closures field becomes pub(super), allowing the sibling old-receiver assertions within canonical_shared to inspect it. This grants no production/public authority and preserves the exact field type and all operations.
- scratch_tests imports the parent's tests alias, so immediate_old_floor's unchanged super::tests path resolves through its immediate parent. Every original assertion and complete old fee/floor body remains byte-exact.

The existing source_reflows_merge test helper acquires a strict whole-source current→7800 preprojection. Its main entry receives raw CI input; its held entry receives raw held/Source36 input. Source36 already passes through held before ours, so ours is deliberately unchanged to avoid applying the same repair after historical projection. The old project function and every existing stage/call remain intact. Unlisted inputs pass through; each listed source must match its complete new SHA and inverse uniquely to its complete frozen7800 predecessor, including the helper itself.

predecessor-projections.json retains complete new/old bytes and SHA for all three changed owners. The three .before.txt files are full frozen Git originals. No old runtime JSON, manifest, source proof, script, archive or historical patch was changed. verify.py checks the exact repair formula, full inverse and all existing38 current→prior source rows, all active merge projection domains and all26 held-auth original inputs. It does not execute Rust or evaluate runtime assertions. source-proof.json records this source-only verification.

No new test selector is introduced. Existing affected selectors and their exact original assertions remain pending corrected-source compilation and execution. No cap, debit, authority, work or diagnostic behavior is changed; no measured gain or qualified runtime result is claimed.
