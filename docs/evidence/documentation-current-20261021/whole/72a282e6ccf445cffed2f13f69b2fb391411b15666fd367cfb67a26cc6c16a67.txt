# Direct borrowed-alias Project graph/Image parity

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only successor to `8bb9f5b7b99e1e71a797460c46180daf716b234b`; one new existing-lib owner, not executed.

The actual canonicalized `alias.current` fixture from the full old/current core control is built as a real Project with the compiler-bundled immutable std.collections provider. Each side calls `project::rebuild_owned_revision` once from independently supplied owned canonical source strings, with no preparatory build, cache warming, quota reset or cap change. A cfg(test) selection hook dispatches the old side at the actual core boundary into the complete independently compiled immediate-old core. The current side remains the ordinary actual producer. A test-only counter is incremented only after an actual FunctionEntry::Borrowed publication; the test rejects a fallback-only success and asserts the old core publishes none.

The new owner compares the complete canonical manifest, source facts and bytes, workspace manifest/revisions, complete semantic graph and its digest, and the actual ProjectSemanticImage bytes/digest. No field or digest normalization is applied. If genuine measured builder billing appears in unequal wires, the complete actual assertion failure is retained before any narrowly justified successor. No parity or closure is claimed before runtime.

The previous complete Drop, frontier, HIR and diagnostic controls remain byte-exact. The original old core body is byte-exact apart from its lexical visibility adapter, followed by isolated test-only selection/count helpers. Every change is cfg(test) behavior or a test-module visibility adapter; no production body or authority changes. All five complete before/current inputs are preserved. Final integration must authenticate its merged raw→this/8bb input stage before the inherited full-source guards; no old protected runtime table is changed here.
